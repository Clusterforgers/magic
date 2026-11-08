//! Goal: `TurnTally::LifeLostMost` is the most life any one named player lost this turn, never the sum over the players, at tables of two, three and four seats, while `TurnTally::LifeLost` keeps the sum.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::Ref;
use mtg_cards::value::{Amount, TurnTally};
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::apply_event;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;

const ME: PlayerId = PlayerId(0);

/// `seats` players; seat `i` lost `lost[i]` life this turn, in two losses when it lost more than one.
fn table(seats: usize, lost: &[u32]) -> GameState {
    let mut game = state(
        (0..seats).map(|_| Vec::new()).collect(),
        (0..seats)
            .map(|n| vec![creature(&format!("C{n}"), 1, 1)])
            .collect(),
    );
    for (seat, &amount) in lost.iter().enumerate() {
        let parts = if amount > 1 {
            vec![1, amount - 1]
        } else {
            vec![amount]
        };
        for part in parts.into_iter().filter(|part| *part > 0) {
            let event = Event::LifeLost {
                player: PlayerId(seat as u64),
                amount: part,
            };
            apply_event(&mut game, &event).unwrap();
        }
    }
    game
}

fn read(game: &GameState, who: Ref, what: TurnTally) -> i64 {
    let source = game.objects().next().unwrap().reference();
    Scope::announcing(game, ME, source)
        .amount(&Amount::Tallied { who, what })
        .unwrap()
}

#[rstest]
#[case::two(2, &[0, 3], Ref::Opponents, 3)]
#[case::three_is_the_max_not_the_sum(3, &[0, 2, 2], Ref::Opponents, 2)]
#[case::four(4, &[5, 1, 4, 2], Ref::Opponents, 4)]
#[case::every_player(4, &[5, 1, 4, 2], Ref::EachPlayer, 5)]
#[case::you(3, &[2, 3, 0], Ref::You, 2)]
#[case::none(3, &[], Ref::EachPlayer, 0)]
fn the_most_life_one_player_lost(
    #[case] seats: usize,
    #[case] lost: &[u32],
    #[case] who: Ref,
    #[case] expected: i64,
) {
    assert_eq!(
        read(&table(seats, lost), who, TurnTally::LifeLostMost),
        expected
    );
}

#[rstest]
#[case::three(3, &[0, 2, 2], 4)]
#[case::four(4, &[5, 1, 4, 2], 7)]
fn the_life_lost_together_stays_the_sum(
    #[case] seats: usize,
    #[case] lost: &[u32],
    #[case] expected: i64,
) {
    assert_eq!(
        read(&table(seats, lost), Ref::Opponents, TurnTally::LifeLost),
        expected
    );
}
