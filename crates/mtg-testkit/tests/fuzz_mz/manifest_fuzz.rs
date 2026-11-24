//! Goal: whole bot games at 2 to 4 seats with the cards that manifest and cloak keep every invariant, and across the seeds each part of the mechanic occurs: a card is manifested and cloaked, a manifested creature card is turned face up, and a manifested permanent dies, is exiled or shows its card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, manifest_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_manifest_and_cloak_keep_every_invariant_and_use_each_part_of_it() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(16);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                manifest_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            // The permanents that lie face down because an effect put them there.
            let mut down: BTreeSet<ObjectId> = BTreeSet::new();
            for event in &outcome.events {
                match event {
                    Event::ZoneChanged {
                        object,
                        to: Zone::Battlefield,
                        cause,
                        ..
                    } if matches!(
                        cause,
                        ZoneCause::Manifested { .. } | ZoneCause::Cloaked { .. }
                    ) =>
                    {
                        down.insert(*object);
                        let part = if matches!(cause, ZoneCause::Cloaked { .. }) {
                            "cloaked"
                        } else {
                            "manifested"
                        };
                        *seen.entry(part).or_default() += 1;
                    }
                    Event::TurnedFaceUp { object, .. } if down.remove(object) => {
                        *seen.entry("turned face up").or_default() += 1;
                    }
                    Event::CreatureDied { object, .. } if down.remove(object) => {
                        *seen.entry("manifested permanent died").or_default() += 1;
                    }
                    Event::ZoneChanged {
                        object, from, to, ..
                    } if *from == Zone::Battlefield && down.remove(object) => {
                        let part = match to {
                            Zone::Exile => "manifested permanent exiled",
                            _ => "manifested card left for another zone",
                        };
                        *seen.entry(part).or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in [
        "manifested",
        "cloaked",
        "turned face up",
        "manifested permanent died",
        "manifested permanent exiled",
    ] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no card went through {part}: {seen:?}"
        );
    }
}
