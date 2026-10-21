//! Goal: the house rules of the turn and life as a random table sets them, for the fuzz games and the properties of every file that plays under house rules.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use mtg_core::house::{EmptyLibrary, Handicap, HouseRules};
use mtg_core::ids::PlayerId;
use mtg_engine::rng::GameRng;
use proptest::prelude::*;

/// Rules of the turn and life, each left out about half the time; the handicaps name seats 0 and 1, which every
/// table has. The values lie where a bot game is cut short or won by life now and then.
pub fn random_turn_rules(rng: &mut GameRng) -> HouseRules {
    let mut some = |bound: u64| (rng.below(2) == 0).then(|| rng.below(bound));
    let (draws, lands, turns, wins, poison) =
        (some(10), some(10), some(40), some(70), some(100));
    let to_u32 = |n: Option<u64>, plus: u64| n.and_then(|n| u32::try_from(n + plus).ok());
    let mut handicaps = Vec::new();
    for seat in 0..2u64 {
        if rng.below(3) == 0 {
            let life = (rng.below(2) == 0).then(|| i32::try_from(rng.below(60) + 1).unwrap_or(1));
            let hand = (rng.below(2) == 0 || life.is_none())
                .then(|| u32::try_from(rng.below(10) + 1).unwrap_or(1));
            handicaps.push(Handicap {
                seat: PlayerId(seat),
                starting_life: life,
                opening_hand_size: hand,
            });
        }
    }
    HouseRules {
        draw_step_cards: to_u32(draws, 1),
        land_plays_per_turn: to_u32(lands, 1),
        mana_burn: (rng.below(3) != 0).then(|| rng.below(2) == 0),
        empty_library: match rng.below(3) {
            0 => Some(EmptyLibrary::Lose),
            1 => Some(EmptyLibrary::ReshuffleGraveyard),
            _ => None,
        },
        turn_limit: to_u32(turns, 1),
        winning_life: wins.and_then(|n| i32::try_from(n + 10).ok()),
        poison_threshold: to_u32(poison, 1),
        handicaps,
        ..HouseRules::default()
    }
}

fn any_handicap(seat: u64) -> impl Strategy<Value = Option<Handicap>> {
    (
        proptest::option::of(1..=60i32),
        proptest::option::of(1..=10u32),
    )
        .prop_map(move |(life, hand)| {
            (life.is_some() || hand.is_some()).then_some(Handicap {
                seat: PlayerId(seat),
                starting_life: life,
                opening_hand_size: hand,
            })
        })
}

/// Rules of the turn and life within their ranges, for a table of two seats or more.
pub fn any_turn_rules() -> impl Strategy<Value = HouseRules> {
    (
        proptest::option::of(1..=10u32),
        proptest::option::of(1..=10u32),
        proptest::option::of(any::<bool>()),
        proptest::option::of(prop_oneof![
            Just(EmptyLibrary::Lose),
            Just(EmptyLibrary::ReshuffleGraveyard)
        ]),
        proptest::option::of(1..=60u32),
        proptest::option::of(10..=80i32),
        proptest::option::of(1..=100u32),
        (any_handicap(0), any_handicap(1)),
    )
        .prop_map(
            |(draws, lands, burn, library, turns, wins, poison, (first, second))| HouseRules {
                draw_step_cards: draws,
                land_plays_per_turn: lands,
                mana_burn: burn,
                empty_library: library,
                turn_limit: turns,
                winning_life: wins,
                poison_threshold: poison,
                handicaps: first.into_iter().chain(second).collect(),
                ..HouseRules::default()
            },
        )
}
