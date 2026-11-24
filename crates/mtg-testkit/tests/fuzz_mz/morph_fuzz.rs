//! Goal: whole bot games at 2 to 4 seats with the cards that have morph and megamorph keep every invariant, and across the seeds each part of the mechanic occurs: a card is cast face down, a face-down permanent is turned face up for the cost of its keyword (a megamorph with its counter), and a face-down permanent dies or is exiled and shows its card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, morph_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_morph_keep_every_invariant_and_use_each_part_of_it() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(16);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                morph_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            // The objects that lie face down on the stack or the battlefield.
            let mut down: BTreeSet<ObjectId> = BTreeSet::new();
            for event in &outcome.events {
                match event {
                    Event::SpellCast {
                        object, choices, ..
                    } if choices.way == Some(CastWay::FaceDown) => {
                        down.insert(*object);
                        *seen.entry("cast face down").or_default() += 1;
                    }
                    Event::TurnedFaceUp { object, .. } => {
                        assert!(
                            down.remove(object),
                            "a permanent that did not lie face down was turned face up (seed {seed}, {players} seats)"
                        );
                        *seen.entry("turned face up").or_default() += 1;
                    }
                    Event::CountersChanged(changed) if changed.delta > 0 => {
                        *seen.entry("counters").or_default() += 1;
                    }
                    Event::CreatureDied { object, .. } if down.remove(object) => {
                        *seen.entry("face-down permanent died").or_default() += 1;
                    }
                    Event::ZoneChanged {
                        object, from, to, ..
                    } if matches!(from, Zone::Battlefield | Zone::Stack) && down.remove(object) => {
                        let part = match to {
                            Zone::Exile => "face-down permanent exiled",
                            Zone::Graveyard(_) => "face-down card put into a graveyard",
                            _ => "face-down card put into a hand or library",
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
        "cast face down",
        "turned face up",
        "counters",
        "face-down permanent died",
        "face-down permanent exiled",
    ] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no card went through {part}: {seen:?}"
        );
    }
}
