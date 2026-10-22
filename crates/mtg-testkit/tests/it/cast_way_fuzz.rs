//! Goal: whole bot games at 2 to 4 seats with the cards that are cast or put onto the battlefield in an unusual way (aftermath) keep every invariant, and across the seeds each way is used: the second half of a split card is cast from a graveyard and exiled, and the first half is cast from a hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, cast_way_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_unusual_cast_ways_keep_every_invariant_and_use_each_way() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                cast_way_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut graveyard_casts = Vec::new();
            for event in &outcome.events {
                match event {
                    // The split cards of the deck have aftermath on their second half and nothing else has a second face.
                    Event::SpellCast {
                        object, choices, ..
                    } if choices.face == Some(1) => {
                        *seen.entry("aftermath cast").or_default() += 1;
                        graveyard_casts.push(*object);
                    }
                    Event::SpellCast { choices, .. } if choices.face == Some(0) => {
                        *seen.entry("first half cast").or_default() += 1;
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Stack,
                        to: Zone::Exile,
                        ..
                    } if graveyard_casts.contains(object) => {
                        *seen.entry("aftermath exile").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for way in ["aftermath cast", "first half cast", "aftermath exile"] {
        assert!(
            seen.get(way).copied().unwrap_or(0) > 0,
            "no card was played the way {way}: {seen:?}"
        );
    }
}
