//! Goal: whole bot games at two, three and four seats with impulse draw cards (Abbot of Keral Keep, Professional Face-Breaker, Reckless Impulse and instants of the tests) keep every invariant and replay to the same state, and across the seeds cards are cast from exile and lands are played from exile, only ever by the player whose library the card came from.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::{PlayOptions, impulse_fuzz_setup, play_game_with};

use crate::common::seeds::for_each_seed;

#[test]
fn bot_games_with_impulse_cards_keep_every_invariant_and_play_cards_from_exile() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let setup = impulse_fuzz_setup(players, seed);
            let outcome = play_game_with(setup.clone(), seed, PlayOptions::default())
                .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            // The cards a library put into exile that no one has moved since, with the player whose library it was.
            let mut exiled = BTreeMap::<ObjectId, PlayerId>::new();
            for event in &outcome.events {
                match event {
                    Event::ZoneChanged {
                        object,
                        from: Zone::Library(owner),
                        to: Zone::Exile,
                        ..
                    } => {
                        exiled.insert(*object, *owner);
                        *seen.entry("exiled from a library").or_default() += 1;
                    }
                    Event::ZoneChanged { object, .. } => {
                        exiled.remove(object);
                    }
                    Event::SpellCast { object, player, .. } if exiled.contains_key(object) => {
                        assert_eq!(exiled.remove(object), Some(*player), "seed {seed}");
                        *seen.entry("cast from exile").or_default() += 1;
                    }
                    Event::LandPlayed { object, player } if exiled.contains_key(object) => {
                        assert_eq!(exiled.remove(object), Some(*player), "seed {seed}");
                        *seen.entry("land played from exile").or_default() += 1;
                    }
                    Event::RuleExpired { .. } => {
                        *seen.entry("permission expired").or_default() += 1;
                    }
                    _ => {}
                }
            }
            let state = replay(&setup, &outcome.events).unwrap();
            assert_eq!(state.state_hash(), outcome.final_hash, "seed {seed}");
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in [
        "exiled from a library",
        "cast from exile",
        "land played from exile",
        "permission expired",
    ] {
        assert!(
            seen.get(what).copied().unwrap_or(0) > 0,
            "never seen: {what}: {seen:?}"
        );
    }
}
