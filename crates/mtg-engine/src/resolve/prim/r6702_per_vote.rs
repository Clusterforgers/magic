//! `Effect::R6702CountersPerVote`: each object of a vote's tally gets a counter for each vote it received.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the tally names, still the object voted for and on the battlefield, gets as many counters as it got
/// votes, all together in the order of the first vote each received; a tally that names nothing is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R6702CountersPerVote { tally, counter } = effect else {
        return Err(wrong_effect("R6702CountersPerVote"));
    };
    let votes = cx
        .resolving
        .bindings()
        .get(tally)
        .cloned()
        .unwrap_or_default();
    let mut counted: Vec<(Target, i32)> = Vec::new();
    for vote in votes {
        match counted.iter_mut().find(|(named, _)| *named == vote) {
            Some((_, count)) => *count = count.saturating_add(1),
            None => counted.push((vote, 1)),
        }
    }
    let state = cx.state();
    let events = counted
        .into_iter()
        .filter(|(named, _)| match named {
            Target::Object(object) => state.object(object.id).is_some_and(|live| {
                live.counter() == object.counter && live.zone() == Zone::Battlefield
            }),
            Target::Player(_) => false,
        })
        .map(|(on, delta)| {
            Event::CountersChanged(CountersChanged {
                on,
                counter: counter.clone(),
                delta,
            })
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: each permanent of a tally gets one counter for each vote it received, in the order of its first vote, a
    //! name never bound is no event, and another effect is refused by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![
                vec![creature("Source", 1, 1)],
                vec![creature("A", 2, 2), creature("B", 2, 2)],
            ],
        )
    }

    fn run_with(game: &GameState, votes: &[usize], json: &str) -> Result<Vec<Event>, ResolveError> {
        let battlefield = game.zone(Zone::Battlefield);
        let source = game.object(battlefield[0]).unwrap();
        let mut resolving = Resolving::begin(source.id());
        let tally: Vec<Target> = votes
            .iter()
            .map(|at| Target::Object(game.object(battlefield[*at]).unwrap().reference()))
            .collect();
        resolving.bindings.insert("votes".to_owned(), tally);
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        match run(&cx, &serde_json::from_str(json).unwrap())? {
            Run::Done { events, .. } => Ok(events),
            _ => panic!("counters per vote ask nothing"),
        }
    }

    const PER_VOTE: &str =
        r#"{"R6702CountersPerVote":{"tally":"votes","counter":{"Other":"stun"}}}"#;

    #[rstest]
    #[case::one_vote_each(&[1, 2], &[(1, 1), (2, 1)])]
    #[case::three_on_one(&[2, 2, 2], &[(2, 3)])]
    #[case::two_and_one(&[2, 1, 2], &[(2, 2), (1, 1)])]
    #[case::no_votes(&[], &[])]
    fn each_permanent_gets_a_counter_per_vote(
        #[case] votes: &[usize],
        #[case] want: &[(usize, i32)],
    ) {
        let game = game();
        let battlefield = game.zone(Zone::Battlefield);
        let got: Vec<(usize, i32)> = run_with(&game, votes, PER_VOTE)
            .unwrap()
            .into_iter()
            .map(|event| match event {
                Event::CountersChanged(changed) => {
                    let Target::Object(on) = changed.on else {
                        panic!("a permanent")
                    };
                    (
                        battlefield.iter().position(|id| *id == on.id).unwrap(),
                        changed.delta,
                    )
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(run_with(&game(), &[], r#"{"Draw":{}}"#).is_err());
    }
}
