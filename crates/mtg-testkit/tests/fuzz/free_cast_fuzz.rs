//! Goal: whole bot Commander games at 2 to 4 seats with the cards that are cast free while their caster controls a commander and the card with strive keep every invariant, and across the seeds each part of the mechanic occurs: a spell is cast without paying its mana cost, the same cards are cast for their mana costs, and a strive spell is cast at two targets or more.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_testkit::{PlayOptions, free_cast_fuzz_setup, play_game_with};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_free_casts_and_strive_keep_every_invariant_and_use_each_part_of_them() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(24);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let setup = free_cast_fuzz_setup(players, seed).unwrap();
            let outcome = play_game_with(setup, seed, PlayOptions::default())
                .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                let Event::SpellCast { choices, .. } = event else {
                    continue;
                };
                match choices.way {
                    Some(CastWay::Free) => *seen.entry("free cast").or_default() += 1,
                    None => *seen.entry("cast for the mana cost").or_default() += 1,
                    Some(_) => {}
                }
                if choices
                    .targets
                    .first()
                    .is_some_and(|first| first.len() >= 2)
                {
                    *seen.entry("strive at two targets").or_default() += 1;
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in [
        "free cast",
        "cast for the mana cost",
        "strive at two targets",
    ] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no spell went through {part}: {seen:?}"
        );
    }
}
