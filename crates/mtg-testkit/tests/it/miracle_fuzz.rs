//! Goal: whole bot games at two, three and four seats with miracle cards (Sister Repentia, Thunderous Wrath, Vanishment) and cards that draw and discard keep every invariant, and across the seeds miracle triggers wait on the stack, cards are cast for their miracle cost and some miracle triggers end with the card still in the hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::zones::Zone;
use mtg_testkit::{PlayOptions, miracle_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_miracle_cards_keep_every_invariant() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(15);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                miracle_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    // The only triggers that work from a hand in this deck are miracle triggers.
                    Event::AbilityTriggered(trigger)
                        if matches!(trigger.snapshot.zone, Zone::Hand(_)) =>
                    {
                        *seen.entry("miracle triggered").or_default() += 1;
                    }
                    Event::SpellCast { choices, .. } if choices.way == Some(CastWay::Miracle) => {
                        *seen.entry("cast for miracle").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for what in ["miracle triggered", "cast for miracle"] {
        assert!(
            seen.get(what).copied().unwrap_or(0) > 0,
            "never seen: {what}: {seen:?}"
        );
    }
    assert!(
        seen["miracle triggered"] > seen["cast for miracle"],
        "some miracle is declined or cannot be paid: {seen:?}"
    );
}
