//! Goal: whole bot games with the cards that are cast for an alternative cost or by a permission (flashback, evoke, overload, casting without paying, as though flash) keep every invariant, and across the seeds each way is used: a spell is flashed back and exiled, evoked, overloaded without a target and cast without paying.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, alternative_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_alternative_costs_keep_every_invariant_and_use_each_way() {
    let seen = std::sync::Mutex::new(BTreeMap::<String, u64>::new());
    let games = fuzz_games(20);
    for_each_seed(0..games, |seed| {
        for players in [2, 4] {
            let outcome = play_game_with(
                alternative_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut flashed = Vec::new();
            for event in &outcome.events {
                match event {
                    Event::SpellCast {
                        choices, object, ..
                    } => {
                        let way = choices.way.map_or("printed", |way| match way {
                            CastWay::Flashback => "flashback",
                            CastWay::Evoke => "evoke",
                            CastWay::Overload => "overload",
                            CastWay::Madness => "madness",
                            CastWay::Miracle => "miracle",
                            CastWay::Suspend => "suspend",
                            CastWay::Foretold => "foretold",
                            CastWay::Escape => "escape",
                            CastWay::Dash => "dash",
                            CastWay::Bestow => "bestow",
                            CastWay::Blitz => "blitz",
                            CastWay::Prowl => "prowl",
                            CastWay::Mayhem => "mayhem",
                            CastWay::Awaken => "awaken",
                            CastWay::JumpStart => "jump-start",
                            CastWay::Retrace => "retrace",
                            CastWay::FaceDown => "face-down",
                            CastWay::Mutate => "mutate",
                            CastWay::Free => "free",
                        });
                        *seen.entry(way.to_owned()).or_default() += 1;
                        if choices.way == Some(CastWay::Flashback) {
                            flashed.push(*object);
                        }
                        if choices.way == Some(CastWay::Overload) {
                            assert!(
                                choices.targets.is_empty(),
                                "an overloaded spell has no target"
                            );
                        }
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Stack,
                        to,
                        ..
                    } if flashed.contains(object) => {
                        assert_eq!(
                            *to,
                            Zone::Exile,
                            "a flashed back spell leaves the stack for exile"
                        );
                        *seen.entry("flashback exiled".to_owned()).or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for way in [
        "printed",
        "flashback",
        "evoke",
        "overload",
        "free",
        "flashback exiled",
    ] {
        assert!(
            seen.get(way).copied().unwrap_or(0) > 0,
            "no spell was cast the way {way}: {seen:?}"
        );
    }
}
