//! Goal: whole bot games at 2 to 4 seats with the cards that have dredge keep every invariant, and across the seeds the mechanic is used: cards are milled, a card returns from a graveyard to a hand by a dredge, and a card that came back that way is cast again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, dredge_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_dredge_keep_every_invariant_and_use_the_mechanic() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(15);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                dredge_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut returned = Vec::new();
            for event in &outcome.events {
                match event {
                    // A card that moves from a graveyard to a hand is a dredge: nothing else of the deck does it.
                    Event::ZoneChanged {
                        object,
                        from: Zone::Graveyard(_),
                        to: Zone::Hand(_),
                        ..
                    } => {
                        *seen.entry("dredge").or_default() += 1;
                        returned.push(*object);
                    }
                    Event::ZoneChanged {
                        from: Zone::Library(_),
                        to: Zone::Graveyard(_),
                        ..
                    } => {
                        *seen.entry("milled").or_default() += 1;
                    }
                    Event::SpellCast { object, .. } if returned.contains(object) => {
                        *seen.entry("cast again").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for step in ["milled", "dredge", "cast again"] {
        assert!(
            seen.get(step).copied().unwrap_or(0) > 0,
            "dredge was never used as {step}: {seen:?}"
        );
    }
}
