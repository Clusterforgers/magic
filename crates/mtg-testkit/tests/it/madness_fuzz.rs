//! Goal: whole bot games at two, three and four seats with madness and suspend cards (Fiery Temper, Basking Rootwalla, Arrogant Wurm, Rift Bolt, Ancestral Vision, Durkwood Baloth) and discard outlets keep every invariant, and across the seeds cards are discarded into exile, cast for their madness cost, suspended and cast from exile for nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, madness_fuzz_setup, play_game_with};

use crate::common::seeds::for_each_seed;

#[test]
fn bot_games_with_madness_and_suspend_cards_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                madness_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::ZoneChanged {
                        from: Zone::Hand(_),
                        to: Zone::Exile,
                        cause: ZoneCause::Discarded,
                        ..
                    } => *seen.entry("discarded into exile").or_default() += 1,
                    Event::ZoneChanged {
                        from: Zone::Hand(_),
                        to: Zone::Exile,
                        cause: ZoneCause::Exiled,
                        ..
                    } => *seen.entry("suspended").or_default() += 1,
                    Event::SpellCast { choices, .. } => match choices.way {
                        Some(CastWay::Madness) => {
                            *seen.entry("cast for madness").or_default() += 1;
                        }
                        Some(CastWay::Suspend) => {
                            *seen.entry("cast from suspend").or_default() += 1;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in [
        "discarded into exile",
        "suspended",
        "cast for madness",
        "cast from suspend",
    ] {
        assert!(
            seen.get(what).copied().unwrap_or(0) > 0,
            "never seen: {what}: {seen:?}"
        );
    }
}
