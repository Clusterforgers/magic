//! Goal: moving counters of any kind moves every counter there is when the count reaches them all ("move all counters"),
//! one kind without a question when only one is held, and else the kinds the controller picks one at a time; each
//! counter taken off the first permanent goes onto the second, and nothing moves from a permanent with none.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use crate::choose::{Choice, Target};
use crate::fixtures::{creature, setup};
use crate::resolve::{Cx, Resolving, Run, Scope};
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// Seat 0 controls the Source, a Wolf with two +1/+1 counters and one charge counter, and a Cat with one shield counter;
/// seat 1 controls a Bear with none.
fn game() -> GameState {
    let mut game = setup(
        vec![vec![], vec![]],
        vec![
            vec![
                creature("Source", 1, 1),
                creature("Wolf", 2, 2),
                creature("Cat", 1, 1),
            ],
            vec![creature("Bear", 2, 2)],
        ],
    );
    game.players[0].battlefield[1].counters =
        vec![(CounterKind::PlusOnePlusOne, 2), (CounterKind::Charge, 1)];
    game.players[0].battlefield[2].counters = vec![(CounterKind::Other("shield".into()), 1)];
    GameState::from_setup(&game).unwrap()
}

fn moving(from: &str, count: &str) -> String {
    format!(
        r#"{{"MoveCounters":{{"from":{{"Each":[{{"Named":"{from}"}}]}},"to":{{"Each":[{{"Named":"Bear"}}]}}{count}}}}}"#
    )
}

/// `None` when a question is asked, else each (holder, kind, delta) proposed.
fn run_with(json: &str, answers: &[usize]) -> Option<Vec<(String, String, i32)>> {
    let game = game();
    let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
    let resolving = Resolving {
        journal: answers
            .iter()
            .map(|index| (ME, Choice::Option(*index)))
            .collect(),
        ..Resolving::begin(source.id())
    };
    let cx = Cx {
        scope: Scope::announcing(&game, ME, source.reference()),
        resolving: &resolving,
    };
    let effect: Effect = serde_json::from_str(json).unwrap();
    match super::move_counters::run(&cx, &effect).unwrap() {
        Run::Ask(_) => None,
        Run::Done { events, .. } => Some(
            events
                .into_iter()
                .map(|event| {
                    let crate::events::Event::CountersChanged(changed) = event else {
                        panic!("{event:?}");
                    };
                    let Target::Object(on) = changed.on else {
                        panic!("objects only");
                    };
                    let holder = game.object(on.id).unwrap().card().name.clone();
                    (holder, changed.counter.name().to_owned(), changed.delta)
                })
                .collect(),
        ),
        Run::Cast(_) => panic!("no cast"),
    }
}

fn change(holder: &str, kind: &str, delta: i32) -> (String, String, i32) {
    (holder.to_owned(), kind.to_owned(), delta)
}

#[rstest]
#[case::all_of_them(
    moving("Wolf", r#","count":{"AnyCounters":{"Each":[{"Named":"Wolf"}]}}"#),
    &[],
    Some(vec![change("Wolf", "+1/+1", -2), change("Bear", "+1/+1", 2), change("Wolf", "charge", -1), change("Bear", "charge", 1)])
)]
#[case::one_kind_held(moving("Cat", ""), &[], Some(vec![change("Cat", "shield", -1), change("Bear", "shield", 1)]))]
#[case::two_kinds_ask(moving("Wolf", ""), &[], None)]
#[case::the_pick(moving("Wolf", ""), &[1], Some(vec![change("Wolf", "charge", -1), change("Bear", "charge", 1)]))]
#[case::none_held(moving("Source", ""), &[], Some(vec![]))]
fn counters_of_any_kind_move(
    #[case] json: String,
    #[case] answers: &[usize],
    #[case] expected: Option<Vec<(String, String, i32)>>,
) {
    let mut found = run_with(&json, answers);
    if let (Some(found), Some(_)) = (found.as_mut(), expected.as_ref()) {
        found.sort();
    }
    let mut expected = expected;
    if let Some(expected) = expected.as_mut() {
        expected.sort();
    }
    assert_eq!(found, expected);
}
