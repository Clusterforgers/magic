//! `Effect::Empower`: the player empowers Jace (Reality Fracture).

use std::sync::Arc;

use mtg_cards::ability::{Ability, LoyaltyCost};
use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;

use super::put_counters::whole;
use super::wrong_effect;
use crate::caps::caps;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::tokens::TokenCreated;

/// The player puts the loyalty counters on a Jace planeswalker token they control (a nontoken Jace never gets them).
/// With none, the blue 0-loyalty Jace token is created first, in a step of its own that binds it and runs the effect
/// again (`Next::Again`), so the counters go on a permanent on the battlefield and a replacement effect sees them;
/// when the token is not there on the rerun (its creation was replaced) nothing more happens. With several Jace tokens
/// the player chooses one.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Empower { count } = effect else {
        return Err(wrong_effect("Empower"));
    };
    Stop::settle(r5508_empower(cx, count))
}

fn r5508_jace() -> Subtype {
    Subtype("Jace".to_owned())
}

/// The name the token of this step is bound to: the step is told apart by its position.
fn r5508_marker(cx: &Cx<'_>) -> String {
    format!("empowered {:?}", cx.resolving.position)
}

fn r5508_empower(cx: &Cx<'_>, count: &Amount) -> Result<Run, Stop> {
    let player = cx.controller();
    let selector = Selector(vec![
        Pred::Type(CardType::Planeswalker),
        Pred::Subtype(r5508_jace()),
        Pred::Token,
        Pred::ControlledBy(Ref::You),
    ]);
    let tokens: Vec<ObjectRef> = cx
        .scope
        .live_objects(&Ref::Each(selector))?
        .into_iter()
        .map(GameObject::reference)
        .collect();
    let marker = r5508_marker(cx);
    if tokens.is_empty() {
        let created = cx.resolving.bindings.contains_key(&marker);
        return Ok(if created {
            Run::events(Vec::new())
        } else {
            r5508_create(cx, marker)
        });
    }
    let mut questions = Questions::new(cx);
    let chosen = if let [only] = tokens.as_slice() {
        *only
    } else {
        let from = tokens.iter().map(|token| token.id).collect();
        let text = "Choose a Jace planeswalker token you control to put the loyalty counters on."
            .to_owned();
        let taken = questions.cards(player, from, (1, 1), text)?;
        taken
            .first()
            .and_then(|id| tokens.iter().find(|token| token.id == *id))
            .copied()
            .ok_or_else(|| ResolveError::Invalid("no Jace token was chosen".into()))?
    };
    questions.end()?;
    let delta = whole(cx.scope.amount(count)?);
    let mut events = Vec::new();
    if delta > 0 {
        events.push(Event::CountersChanged(CountersChanged {
            on: Target::Object(chosen),
            counter: CounterKind::Loyalty,
            delta,
        }));
    }
    Ok(Run::events(events))
}

/// The blue Jace planeswalker token with 0 loyalty, "[-1]: Surveil 1" and "[-3]: Draw a card".
pub(crate) fn r5508_jace_token() -> TokenSpec {
    let loyalty = |minus: u32, json: &str| Ability::Loyalty {
        cost: LoyaltyCost::Minus(minus),
        targets: Vec::new(),
        effects: serde_json::from_str(json).unwrap_or_default(),
    };
    TokenSpec {
        name: "Jace".to_owned(),
        colors: vec![Color::Blue],
        supertypes: Vec::new(),
        types: vec![CardType::Planeswalker],
        subtypes: vec![r5508_jace()],
        power: None,
        toughness: None,
        abilities: vec![
            loyalty(1, r#"[{"Surveil":{"count":1}}]"#),
            loyalty(3, r#"[{"Draw":{}}]"#),
        ],
        mana_cost: None,
    }
}

/// The step that creates the Jace token and binds it, then runs the effect again.
fn r5508_create(cx: &Cx<'_>, marker: String) -> Run {
    let definition = Arc::new(token_definition(&r5508_jace_token(), &caps()));
    let object = cx.state().next_object();
    let token = Event::TokenCreated(TokenCreated {
        object,
        copy_of: None,
        controller: cx.controller(),
        name: definition.name.clone(),
        tapped: false,
        attacking: None,
        blocking: None,
        definition,
    });
    // A token is a new object: its zone-change counter starts at 0.
    let made = ObjectRef {
        id: object,
        counter: 0,
    };
    Run::Done {
        events: vec![token],
        bound: vec![(marker, vec![Target::Object(made)])],
        snapshots: Vec::new(),
        next: Next::Again,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an empower with no Jace token creates the blue Jace planeswalker token with its two loyalty abilities and runs again; with one Jace token (the only one, or the one chosen among several) it puts that many loyalty counters on it; a nontoken Jace and an opponent's Jace token never get them; nothing more happens when the token it made is not there.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// A Jace planeswalker token named `name` under `controller`, folded into the game.
    fn add_token(game: &mut GameState, controller: PlayerId, name: &str) {
        let mut spec = r5508_jace_token();
        spec.name = name.to_owned();
        let created = TokenCreated {
            object: game.next_object(),
            copy_of: None,
            controller,
            name: name.to_owned(),
            tapped: false,
            attacking: None,
            blocking: None,
            definition: Arc::new(token_definition(&spec, &caps())),
        };
        apply_event(game, &Event::TokenCreated(created)).unwrap();
    }

    /// Seat 0 controls the Source, a nontoken Jace and `mine` Jace tokens; seat 1 controls a Jace token.
    fn game(mine: usize) -> GameState {
        let printed = Arc::new(
            mtg_cards::CardDefinition::new("Jace, Printed")
                .with_type(CardType::Planeswalker)
                .with_subtype("Jace"),
        );
        let mut game = GameState::from_setup(&setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1), printed], vec![]],
        ))
        .unwrap();
        add_token(&mut game, PlayerId(1), "Their Jace");
        for n in 0..mine {
            add_token(&mut game, ME, &format!("Jace {n}"));
        }
        game
    }

    fn begun(game: &GameState) -> Resolving {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        Resolving::begin(source.id())
    }

    fn run_with(game: &GameState, resolving: &Resolving, count: u32) -> Result<Run, ResolveError> {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving,
        };
        run(
            &cx,
            &Effect::Empower {
                count: Amount::Fixed(i32::try_from(count).unwrap()),
            },
        )
    }

    fn loyalty_on(events: &[Event]) -> Vec<(String, i32)> {
        events
            .iter()
            .map(|event| match event {
                Event::CountersChanged(CountersChanged {
                    on: Target::Object(on),
                    counter: CounterKind::Loyalty,
                    delta,
                }) => (on.id, *delta),
                other => panic!("unexpected {other:?}"),
            })
            .map(|(id, delta)| (format!("{id:?}"), delta))
            .collect()
    }

    #[test]
    fn with_no_jace_token_the_token_is_made_and_the_effect_runs_again() {
        let game = game(0);
        let Run::Done {
            events,
            bound,
            next,
            ..
        } = run_with(&game, &begun(&game), 5).unwrap()
        else {
            panic!("done");
        };
        assert_eq!(next, Next::Again);
        let [Event::TokenCreated(token)] = events.as_slice() else {
            panic!("one token: {events:?}");
        };
        assert_eq!((token.name.as_str(), token.controller), ("Jace", ME));
        assert_eq!(token.definition.subtypes, vec![r5508_jace()]);
        assert_eq!(bound.len(), 1);
    }

    #[test]
    fn the_token_has_its_two_loyalty_abilities_and_no_loyalty() {
        let spec = r5508_jace_token();
        let costs: Vec<_> = spec
            .abilities
            .iter()
            .map(|ability| match ability {
                Ability::Loyalty { cost, effects, .. } => (cost.clone(), effects.len()),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            costs,
            vec![(LoyaltyCost::Minus(1), 1), (LoyaltyCost::Minus(3), 1)]
        );
        assert_eq!(spec.types, vec![CardType::Planeswalker]);
        assert!(spec.supertypes.is_empty(), "the token is not legendary");
    }

    #[test]
    fn when_the_token_is_not_there_on_the_rerun_nothing_more_happens() {
        let game = game(0);
        let mut resolving = begun(&game);
        let name = r5508_marker(&Cx {
            scope: Scope::announcing(&game, ME, game.objects().next().unwrap().reference()),
            resolving: &resolving,
        });
        resolving.bindings.insert(name, Vec::new());
        let Run::Done { events, next, .. } = run_with(&game, &resolving, 2).unwrap() else {
            panic!("done");
        };
        assert!(events.is_empty() && next == Next::Sibling);
    }

    #[rstest]
    #[case::two(2, 1)]
    #[case::five(5, 1)]
    #[case::none(0, 0)]
    fn the_only_jace_token_gets_the_counters(#[case] count: u32, #[case] changes: usize) {
        let game = game(1);
        let Run::Done { events, next, .. } = run_with(&game, &begun(&game), count).unwrap() else {
            panic!("done");
        };
        assert_eq!(next, Next::Sibling);
        let token = game.objects().find(|o| o.card().name == "Jace 0").unwrap();
        let on = loyalty_on(&events);
        assert_eq!(on.len(), changes);
        for (id, delta) in on {
            assert_eq!(
                (id, delta),
                (format!("{:?}", token.id()), i32::try_from(count).unwrap())
            );
        }
    }

    #[test]
    fn several_jace_tokens_ask_and_the_one_chosen_gets_the_counters() {
        let game = game(2);
        let Run::Ask(decision) = run_with(&game, &begun(&game), 3).unwrap() else {
            panic!("the player chooses");
        };
        let crate::decision::DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("a choice of cards");
        };
        assert_eq!(
            options.from.len(),
            2,
            "neither the printed Jace nor the opponent's token is offered"
        );
        let resolving = Resolving {
            journal: vec![(ME, Choice::Cards(vec![options.from[1]]))],
            ..begun(&game)
        };
        let Run::Done { events, .. } = run_with(&game, &resolving, 3).unwrap() else {
            panic!("done");
        };
        assert_eq!(
            loyalty_on(&events),
            vec![(format!("{:?}", options.from[1]), 3)]
        );
    }

    #[test]
    fn another_effect_is_refused() {
        let game = game(1);
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = begun(&game);
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        assert!(run(&cx, &serde_json::from_str(r#"{"Draw":{}}"#).unwrap()).is_err());
    }
}
