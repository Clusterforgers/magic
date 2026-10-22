//! `Effect::Fabricate`: the source's controller puts +1/+1 counters on it or creates Servo tokens (CR 702.123).

use mtg_cards::card::TokenSpec;
use mtg_cards::effect::{Effect, TokenRef};
use mtg_cards::refs::Ref;
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::{CardType, Subtype};

use super::explore::actors;
use super::put_counters::whole;
use super::wrong_effect;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The controller of a source that is still on the battlefield chooses (`YesNo`, "you may") between the counters, as a
/// proposed `CountersChanged` that a replacement effect sees (Hardened Scales), and the tokens. A source that has left
/// the battlefield cannot take counters (CR 702.123b), so the tokens are made without a question; one that cannot be
/// found does nothing. The tokens are `CreateToken` events too, which Doubling Season and the like replace.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Fabricate { count } = effect else {
        return Err(wrong_effect("Fabricate"));
    };
    Stop::settle(fabricate(cx, count))
}

fn fabricate(cx: &Cx<'_>, count: &Amount) -> Result<Run, Stop> {
    let number = cx.scope.amount(count)?;
    let delta = whole(number);
    let Some(source) = actors(cx, &Ref::This)?.into_iter().next() else {
        return Ok(Run::events(Vec::new()));
    };
    if delta == 0 {
        return Ok(Run::events(Vec::new()));
    }
    let Some(holder) = source.permanent else {
        return Ok(servos(cx, count)?);
    };
    let mut questions = Questions::new(cx);
    let text = format!(
        "Put {delta} +1/+1 counter{} on this creature? If not, create {delta} 1/1 Servo token{}.",
        plural(delta),
        plural(delta)
    );
    let counters = questions.yes_no(source.player, text)?;
    questions.end()?;
    if counters {
        Ok(Run::events(vec![Event::CountersChanged(CountersChanged {
            on: Target::Object(holder),
            counter: CounterKind::PlusOnePlusOne,
            delta,
        })]))
    } else {
        Ok(servos(cx, count)?)
    }
}

const fn plural(number: i32) -> &'static str {
    if number == 1 { "" } else { "s" }
}

/// The step that creates the 1/1 colorless Servo artifact creature tokens.
fn servos(cx: &Cx<'_>, count: &Amount) -> Result<Run, ResolveError> {
    let spec = TokenSpec {
        name: "Servo".to_owned(),
        colors: Vec::new(),
        supertypes: Vec::new(),
        types: vec![CardType::Artifact, CardType::Creature],
        subtypes: vec![Subtype("Servo".to_owned())],
        power: Some(1),
        toughness: Some(1),
        abilities: Vec::new(),
    };
    let make = Effect::CreateToken {
        who: Ref::You,
        token: TokenRef::Custom(Box::new(spec)),
        count: count.clone(),
        tapped: false,
        attacking: false,
        bind: None,
    };
    super::create_token::run(cx, &make)
}

#[cfg(test)]
mod tests {
    //! Goal: fabricating on a source that is on the battlefield asks a yes-or-no question, puts the counters on Yes and makes the Servo tokens on No; on a source that has left the battlefield it makes the tokens without a question; a count of 0 does nothing; and it refuses another effect.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game(on_board: bool) -> GameState {
        let source = vec![creature("Source", 1, 1)];
        let (hand, board) = if on_board {
            (vec![], source)
        } else {
            (source, vec![])
        };
        GameState::from_setup(&setup(vec![hand, vec![]], vec![board, vec![]])).unwrap()
    }

    fn run_with(game: &GameState, json: &str, journal: Vec<(PlayerId, Choice)>) -> Run {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap()).unwrap()
    }

    fn events(run: Run) -> Vec<Event> {
        match run {
            Run::Done { events, .. } => events,
            other => panic!("done, got {other:?}"),
        }
    }

    const TWO: &str = r#"{"Fabricate":{"count":2}}"#;

    #[test]
    fn a_source_on_the_battlefield_asks_whether_to_take_the_counters() {
        let game = game(true);
        assert!(matches!(
            run_with(&game, TWO, vec![]),
            Run::Ask(decision) if decision.player == ME
        ));
    }

    #[rstest]
    #[case::two_counters(2, 1)]
    #[case::one_counter(1, 1)]
    fn yes_puts_the_counters_on_the_source(#[case] count: i32, #[case] expected: usize) {
        let game = game(true);
        let json = format!(r#"{{"Fabricate":{{"count":{count}}}}}"#);
        let found = events(run_with(&game, &json, vec![(ME, Choice::YesNo(true))]));
        assert_eq!(found.len(), expected);
        assert!(matches!(
            &found[0],
            Event::CountersChanged(change)
                if change.delta == count && change.counter == CounterKind::PlusOnePlusOne
        ));
    }

    #[test]
    fn no_makes_that_many_servos() {
        let game = game(true);
        let found = events(run_with(&game, TWO, vec![(ME, Choice::YesNo(false))]));
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|event| matches!(
            event,
            Event::TokenCreated(token) if token.name == "Servo" && token.controller == ME
        )));
    }

    #[test]
    fn a_source_that_left_makes_the_servos_without_a_question() {
        let game = game(false);
        let found = events(run_with(&game, TWO, vec![]));
        assert_eq!(
            found.len(),
            0,
            "a card in the hand is not a creature that left"
        );
    }

    #[test]
    fn a_count_of_zero_does_nothing() {
        let game = game(true);
        assert_eq!(
            events(run_with(&game, r#"{"Fabricate":{"count":0}}"#, vec![])).len(),
            0
        );
    }

    #[test]
    fn another_effect_is_refused() {
        let game = game(true);
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let other: Effect = serde_json::from_str(r#"{"Adapt":{"count":1}}"#).unwrap();
        assert!(run(&cx, &other).is_err());
    }
}
