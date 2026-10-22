//! `Effect::Amass`: the player amasses (CR 701.47).

use std::sync::Arc;

use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::EffectId;

use super::put_counters::whole;
use super::wrong_effect;
use crate::caps::caps;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::effects::{ContinuousEffect, Modification, Selector as Picked};
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::tokens::TokenCreated;

/// The player puts the counters on an Army creature they control and the Army becomes a creature of the subtype
/// too (a continuous effect without end). With no Army they control, a 0/0 black Army token of the subtype is created
/// first: that is a step of its own that binds the token and runs the effect again (`Next::Again`), so the counters
/// are put on a creature that is on the battlefield and a replacement effect sees them (Hardened Scales). With several
/// Armies the player chooses one. When the token is not there on the rerun (its creation was replaced) the effect does
/// nothing more. One `Amassed` follows the counters, even when there are none.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Amass { subtype, count } = effect else {
        return Err(wrong_effect("Amass"));
    };
    Stop::settle(amass(cx, subtype, count))
}

fn army() -> Subtype {
    Subtype("Army".to_owned())
}

/// The name the token of this step is bound to: the step is told apart by its position.
fn marker(cx: &Cx<'_>) -> String {
    format!("amassed {:?}", cx.resolving.position)
}

fn amass(cx: &Cx<'_>, subtype: &Subtype, count: &Amount) -> Result<Run, Stop> {
    let player = cx.controller();
    let selector = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::Subtype(army()),
        Pred::ControlledBy(Ref::You),
    ]);
    let armies: Vec<ObjectRef> = cx
        .scope
        .live_objects(&Ref::Each(selector))?
        .into_iter()
        .map(GameObject::reference)
        .collect();
    let marker = marker(cx);
    if armies.is_empty() {
        let created = cx.resolving.bindings.contains_key(&marker);
        return Ok(if created {
            Run::events(Vec::new())
        } else {
            create(cx, subtype, marker)
        });
    }
    let mut questions = Questions::new(cx);
    let chosen = if let [only] = armies.as_slice() {
        *only
    } else {
        let from = armies.iter().map(|army| army.id).collect();
        let text = "Choose an Army creature you control to put the counters on.".to_owned();
        let taken = questions.cards(player, from, (1, 1), text)?;
        taken
            .first()
            .and_then(|id| armies.iter().find(|army| army.id == *id))
            .copied()
            .ok_or_else(|| ResolveError::Invalid("no Army was chosen".into()))?
    };
    questions.end()?;
    let state = cx.state();
    let mut events = Vec::new();
    let has = Selector(vec![Pred::Subtype(subtype.clone())]);
    if !cx.scope.selects(&has, chosen)? {
        events.push(Event::EffectCreated {
            effect: ContinuousEffect::new(
                EffectId(state.next_effect),
                Picked::Objects(vec![chosen]),
                Modification::AddSubtype(subtype.clone()),
                Duration::Forever,
                state.next_timestamp,
            )
            .from_source(cx.scope.source)
            .begun(player, state.turn().number()),
        });
    }
    let delta = whole(cx.scope.amount(count)?);
    if delta > 0 {
        events.push(Event::CountersChanged(CountersChanged {
            on: Target::Object(chosen),
            counter: CounterKind::PlusOnePlusOne,
            delta,
        }));
    }
    events.push(Event::Amassed {
        object: chosen.id,
        player,
    });
    Ok(Run::events(events))
}

/// The step that creates the 0/0 black Army token and binds it, then runs the effect again.
fn create(cx: &Cx<'_>, subtype: &Subtype, marker: String) -> Run {
    let mut subtypes = vec![subtype.clone()];
    if *subtype != army() {
        subtypes.push(army());
    }
    let spec = TokenSpec {
        name: format!("{} Army", subtype.0),
        colors: vec![Color::Black],
        supertypes: Vec::new(),
        types: vec![CardType::Creature],
        subtypes,
        power: Some(0),
        toughness: Some(0),
        abilities: Vec::new(),
    };
    let definition = Arc::new(token_definition(&spec, &caps()));
    let object = cx.state().next_object();
    let token = Event::TokenCreated(TokenCreated {
        object,
        copy_of: None,
        controller: cx.controller(),
        name: definition.name.clone(),
        tapped: false,
        attacking: None,
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
    //! Goal: an amass with no Army creates the black Army token of the subtype, binds it and asks to run again; with an Army (the only one, or the one chosen among several) it adds the subtype the Army lacks, the counters and one `Amassed`; it does nothing more when the token it made is not there; and it refuses another effect and an answer nobody asked for.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const EFFECT: &str = r#"{"Amass":{"subtype":"Orc","count":2}}"#;

    /// Seat 0 controls the Source and `armies` Zombie Army creatures; seat 1 controls an Orc Army.
    fn game(armies: usize) -> GameState {
        let army = |name: &str, subtype: &str| {
            Arc::new(
                mtg_cards::CardDefinition::new(name)
                    .with_type(CardType::Creature)
                    .with_subtype("Army")
                    .with_subtype(subtype)
                    .with_power_toughness(0, 0),
            )
        };
        let mut mine = vec![creature("Source", 1, 1)];
        mine.extend((0..armies).map(|n| army(&format!("Army {n}"), "Zombie")));
        GameState::from_setup(&setup(
            vec![vec![], vec![]],
            vec![mine, vec![army("Their Army", "Orc")]],
        ))
        .unwrap()
    }

    fn begun(game: &GameState) -> Resolving {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        Resolving::begin(source.id())
    }

    fn run_with(game: &GameState, resolving: &Resolving, json: &str) -> Result<Run, ResolveError> {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    #[test]
    fn with_no_army_the_token_is_made_and_the_effect_runs_again() {
        let game = game(0);
        let Run::Done {
            events,
            bound,
            next,
            ..
        } = run_with(&game, &begun(&game), EFFECT).unwrap()
        else {
            panic!("done");
        };
        assert_eq!(next, Next::Again);
        let [Event::TokenCreated(token)] = events.as_slice() else {
            panic!("one token: {events:?}");
        };
        assert_eq!(
            (token.name.as_str(), token.controller, token.object),
            ("Orc Army", ME, game.next_object())
        );
        assert_eq!(bound.len(), 1);
        assert_eq!(bound[0].1.len(), 1);
    }

    #[test]
    fn when_the_token_is_not_there_on_the_rerun_nothing_more_happens() {
        let game = game(0);
        let mut resolving = begun(&game);
        let name = marker(&Cx {
            scope: Scope::announcing(&game, ME, game.objects().next().unwrap().reference()),
            resolving: &resolving,
        });
        resolving.bindings.insert(name, Vec::new());
        let Run::Done { events, next, .. } = run_with(&game, &resolving, EFFECT).unwrap() else {
            panic!("done");
        };
        assert!(events.is_empty() && next == Next::Sibling);
    }

    #[rstest]
    #[case::the_subtype_it_lacks(r#"{"Amass":{"subtype":"Orc","count":2}}"#, &["effect", "+2", "amassed"])]
    #[case::the_subtype_it_has(r#"{"Amass":{"subtype":"Zombie","count":3}}"#, &["+3", "amassed"])]
    #[case::no_counters(r#"{"Amass":{"subtype":"Zombie","count":0}}"#, &["amassed"])]
    fn the_only_army_gets_the_subtype_and_the_counters(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        let game = game(1);
        let Run::Done { events, next, .. } = run_with(&game, &begun(&game), json).unwrap() else {
            panic!("done");
        };
        assert_eq!(next, Next::Sibling);
        let words: Vec<String> = events
            .iter()
            .map(|event| match event {
                Event::EffectCreated { .. } => "effect".to_owned(),
                Event::CountersChanged(changed) => format!("{:+}", changed.delta),
                Event::Amassed { player, .. } => {
                    assert_eq!(*player, ME);
                    "amassed".to_owned()
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(words, expected);
    }

    #[test]
    fn several_armies_ask_and_the_army_chosen_gets_the_counters() {
        let game = game(2);
        let Run::Ask(decision) = run_with(&game, &begun(&game), EFFECT).unwrap() else {
            panic!("the player chooses");
        };
        assert_eq!(decision.player, ME);
        let crate::decision::DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("a choice of cards");
        };
        assert_eq!(options.from.len(), 2, "the opponent's Army is not offered");
        let resolving = Resolving {
            journal: vec![(ME, Choice::Cards(vec![options.from[1]]))],
            ..begun(&game)
        };
        let Run::Done { events, .. } = run_with(&game, &resolving, EFFECT).unwrap() else {
            panic!("done");
        };
        let held: Vec<ObjectId> = events
            .iter()
            .filter_map(|event| match event {
                Event::CountersChanged(CountersChanged {
                    on: Target::Object(held),
                    ..
                }) => Some(held.id),
                _ => None,
            })
            .collect();
        assert_eq!(held, [options.from[1]]);
    }

    #[test]
    fn an_answer_nobody_asked_for_and_another_effect_are_refused() {
        let game = game(1);
        let resolving = Resolving {
            journal: vec![(ME, Choice::YesNo(true))],
            ..begun(&game)
        };
        assert!(run_with(&game, &resolving, EFFECT).is_err());
        assert!(run_with(&game, &begun(&game), r#"{"Draw":{}}"#).is_err());
    }
}
