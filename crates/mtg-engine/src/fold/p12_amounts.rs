//! Goal: `Amount::HighestCounters` is the most counters of the kind any one named player holds, never their sum, never the counters of another kind, and zero when no player is named, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::Ref;
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::apply_event;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;

const ME: PlayerId = PlayerId(0);

/// `seats` players; `poison[i]` poison counters on seat `i` and one charge counter on every seat.
fn table(seats: usize, poison: &[i32]) -> GameState {
    let mut game = state(
        (0..seats).map(|_| Vec::new()).collect(),
        (0..seats)
            .map(|n| vec![creature(&format!("C{n}"), 1, 1)])
            .collect(),
    );
    for seat in 0..seats {
        for (kind, delta) in [
            (CounterKind::Poison, poison.get(seat).copied().unwrap_or(0)),
            (CounterKind::Charge, 1),
        ] {
            if delta > 0 {
                let event = Event::CountersChanged(CountersChanged {
                    on: Target::Player(PlayerId(seat as u64)),
                    counter: kind,
                    delta,
                });
                apply_event(&mut game, &event).unwrap();
            }
        }
    }
    game
}

fn highest(game: &GameState, who: Ref, counter: CounterKind) -> i64 {
    let source = game.objects().next().unwrap().reference();
    Scope::announcing(game, ME, source)
        .amount(&Amount::HighestCounters { who, counter })
        .unwrap()
}

#[rstest]
#[case::two(2, &[0, 3], 3)]
#[case::three_is_the_max_not_the_sum(3, &[0, 2, 3], 3)]
#[case::four(4, &[0, 1, 4, 2], 4)]
#[case::none(3, &[], 0)]
fn the_most_poison_of_the_opponents(
    #[case] seats: usize,
    #[case] poison: &[i32],
    #[case] expected: i64,
) {
    let game = table(seats, poison);
    assert_eq!(
        highest(&game, Ref::Opponents, CounterKind::Poison),
        expected
    );
}

#[rstest]
fn the_caster_is_not_one_of_their_opponents(#[values(2, 3, 4)] seats: usize) {
    let game = table(seats, &[9, 1, 1, 1]);
    assert_eq!(highest(&game, Ref::Opponents, CounterKind::Poison), 1);
    assert_eq!(highest(&game, Ref::EachPlayer, CounterKind::Poison), 9);
    assert_eq!(highest(&game, Ref::You, CounterKind::Poison), 9);
}

#[rstest]
fn another_kind_of_counter_is_not_counted(#[values(2, 3, 4)] seats: usize) {
    let game = table(seats, &[0, 5, 5, 5]);
    assert_eq!(highest(&game, Ref::Opponents, CounterKind::Charge), 1);
}
