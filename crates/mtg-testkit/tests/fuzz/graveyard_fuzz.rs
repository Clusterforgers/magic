//! Goal: whole bot games at 2 to 4 seats with the cards that return from a graveyard or exile (unearth, escape, rebound) keep every invariant, and across the seeds each way is used: a card is unearthed and exiled, a spell escapes, and a spell is exiled by rebound and cast again from exile.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, graveyard_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_graveyard_casts_keep_every_invariant_and_use_each_way() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(15);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                graveyard_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::AbilityPutOnStack(put)
                        if matches!(put.snapshot.zone, Zone::Graveyard(_)) =>
                    {
                        *seen.entry("unearth").or_default() += 1;
                    }
                    Event::SpellCast { choices, .. } if choices.way == Some(CastWay::Escape) => {
                        *seen.entry("escape").or_default() += 1;
                    }
                    Event::SpellCast { choices, .. } if choices.way == Some(CastWay::Free) => {
                        *seen.entry("rebound cast").or_default() += 1;
                    }
                    Event::DelayedTriggerCreated(created)
                        if created.snapshot.zone == Zone::Exile =>
                    {
                        *seen.entry("rebound").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for way in ["unearth", "escape", "rebound", "rebound cast"] {
        assert!(
            seen.get(way).copied().unwrap_or(0) > 0,
            "no card was played the way {way}: {seen:?}"
        );
    }
}
