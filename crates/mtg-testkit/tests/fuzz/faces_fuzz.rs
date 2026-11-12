//! Goal: whole bot games at two, three and four seats with a split card, a fuse card, an Adventure, two modal double-faced cards and transforming cards (an upkeep trigger, an activated ability, daybound) keep every invariant, and across the seeds the faces are cast (a half, the Adventure, a back face), back face lands are played, Adventures are exiled on an adventure and cast again as the creature, permanents are turned over, the day or night begins, and a log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::Event;
use mtg_engine::events::ZoneCause;
use mtg_testkit::{PlayOptions, face_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_cards_of_several_faces_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(15);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome =
                play_game_with(face_fuzz_setup(players, seed), seed, PlayOptions::default())
                    .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = mtg_engine::replay(&face_fuzz_setup(players, seed), &outcome.events)
                .unwrap_or_else(|error| panic!("seed {seed}: the log does not replay: {error}"));
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::SpellCast { choices, .. } if choices.face.is_some() => {
                        *seen.entry("cast as a face").or_default() += 1;
                    }
                    Event::ZoneChanged {
                        cause: ZoneCause::Adventure,
                        ..
                    } => *seen.entry("exiled on an adventure").or_default() += 1,
                    Event::LandPlayed { face: Some(_), .. } => {
                        *seen.entry("played as a back face land").or_default() += 1;
                    }
                    Event::Transformed { .. } => *seen.entry("turned over").or_default() += 1,
                    Event::DayNightChanged { .. } => {
                        *seen.entry("day or night began").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in [
        "cast as a face",
        "exiled on an adventure",
        "turned over",
        "played as a back face land",
        "day or night began",
    ] {
        assert!(
            seen.contains_key(what),
            "no game had a card {what}: {seen:?}"
        );
    }
}
