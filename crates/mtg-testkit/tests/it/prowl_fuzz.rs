//! Goal: whole bot games at 2 to 4 seats with the cards that have prowl keep every invariant, and across the seeds each part of the mechanic occurs: a spell is cast for its prowl cost, only in a turn in which a player was dealt combat damage, and a creature cast that way draws its controller a card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::combat::DamageTarget;
use mtg_engine::turn::Step;
use mtg_testkit::{PlayOptions, play_game_with, prowl_fuzz_setup};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_prowl_keep_every_invariant_and_use_each_part_of_it() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(16);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                prowl_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            let mut step = Step::Untap;
            // Combat damage to a player in the turn so far.
            let mut damaged = false;
            // The seats that cast a spell for its prowl cost in this turn: their draw is still to come.
            let mut owed: Vec<usize> = Vec::new();
            for event in &outcome.events {
                match event {
                    Event::StepBegan { step: began } => {
                        step = *began;
                        if *began == Step::Untap {
                            damaged = false;
                            owed.clear();
                        }
                    }
                    Event::DamageDealt {
                        target: DamageTarget::Player(_),
                        amount,
                        ..
                    } if *amount > 0
                        && matches!(step, Step::FirstStrikeDamage | Step::CombatDamage) =>
                    {
                        damaged = true;
                    }
                    Event::SpellCast {
                        player, choices, ..
                    } if choices.way == Some(CastWay::Prowl) => {
                        assert!(
                            damaged,
                            "a spell was cast for its prowl cost with no combat damage dealt to a player this turn (seed {seed}, {players} seats)"
                        );
                        *seen.entry("prowl cast").or_default() += 1;
                        owed.push(player.index());
                    }
                    Event::CardDrawn { player, .. } if owed.contains(&player.index()) => {
                        owed.retain(|p| *p != player.index());
                        *seen.entry("prowl draw").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in ["prowl cast", "prowl draw"] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no card went through {part}: {seen:?}"
        );
    }
}
