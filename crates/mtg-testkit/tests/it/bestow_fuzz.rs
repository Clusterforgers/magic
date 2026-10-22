//! Goal: whole bot games at 2 to 4 seats with the cards that have bestow keep every invariant, and across the seeds each part of bestow happens: a card is cast for its bestow cost, enters attached to a creature, becomes a creature again when its host leaves, and resolves as a creature when its target is gone; a bestowed spell has exactly one target.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_testkit::{PlayOptions, bestow_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_bestow_cards_keep_every_invariant_and_use_each_part_of_bestow() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(150);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                bestow_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut bestowed = Vec::new();
            for (at, event) in outcome.events.iter().enumerate() {
                match event {
                    Event::SpellCast {
                        object, choices, ..
                    } if choices.way == Some(CastWay::Bestow) => {
                        *seen.entry("bestow cast").or_default() += 1;
                        assert_eq!(
                            choices.targets.len(),
                            1,
                            "a bestowed spell has one target (seed {seed}, {players} seats)"
                        );
                        bestowed.push(*object);
                    }
                    Event::SpellResolved { object } if bestowed.contains(object) => {
                        match outcome.events.get(at + 1) {
                            Some(Event::Attached { object: next, .. }) if next == object => {
                                *seen.entry("bestow attached").or_default() += 1;
                            }
                            _ => {
                                *seen.entry("bestow resolved as a creature").or_default() += 1;
                                bestowed.retain(|bestowed| bestowed != object);
                            }
                        }
                    }
                    Event::Unattached { object } if bestowed.contains(object) => {
                        *seen.entry("bestow ended").or_default() += 1;
                        bestowed.retain(|bestowed| bestowed != object);
                    }
                    Event::ZoneChanged { object, .. } => {
                        bestowed.retain(|bestowed| bestowed != object);
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in [
        "bestow cast",
        "bestow attached",
        "bestow ended",
        "bestow resolved as a creature",
    ] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "bestow never did: {part}: {seen:?}"
        );
    }
}
