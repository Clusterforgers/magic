//! Goal: whole bot games at 2 to 4 seats with the cards that are cast or put onto the battlefield in an unusual way (aftermath, dash) keep every invariant, and across the seeds each way is used: the second half of a split card is cast from a graveyard and exiled, and the first half is cast from a hand; a creature is cast for its dash cost and returns to its owner's hand, and none is still in play when the turn is cleaned up.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, cast_way_fuzz_setup, play_game_with};

use crate::common::seeds::for_each_seed;

#[test]
fn bot_games_with_unusual_cast_ways_keep_every_invariant_and_use_each_way() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
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
            let mut dashed = Vec::new();
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
                    Event::SpellCast {
                        object, choices, ..
                    } if choices.way == Some(CastWay::Dash) => {
                        *seen.entry("dash cast").or_default() += 1;
                        dashed.push(*object);
                    }
                    Event::ZoneChanged {
                        object, from, to, ..
                    } if dashed.contains(object)
                        && !(*from == Zone::Stack && *to == Zone::Battlefield) =>
                    {
                        dashed.retain(|dashed| dashed != object);
                        if *from == Zone::Battlefield && matches!(to, Zone::Hand(_)) {
                            *seen.entry("dash return").or_default() += 1;
                        }
                    }
                    Event::CreatureDied { object, .. } => dashed.retain(|dashed| dashed != object),
                    Event::StepBegan {
                        step: Step::Cleanup,
                    } => {
                        assert_eq!(
                            dashed,
                            [],
                            "a dashed creature is still in play at cleanup (seed {seed}, {players} seats)"
                        );
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
    for way in [
        "aftermath cast",
        "first half cast",
        "aftermath exile",
        "dash cast",
        "dash return",
    ] {
        assert!(
            seen.get(way).copied().unwrap_or(0) > 0,
            "no card was played the way {way}: {seen:?}"
        );
    }
}
