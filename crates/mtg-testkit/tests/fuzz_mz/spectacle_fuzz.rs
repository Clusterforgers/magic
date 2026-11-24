//! Goal: whole bot games at 2 to 4 seats with the cards that have spectacle, afterlife and horsemanship keep every invariant, and across the seeds each part of the mechanics occurs: a spell is cast for its spectacle cost only in a turn in which an opponent of its caster lost life, and a creature with afterlife that dies leaves Spirit tokens.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_testkit::{PlayOptions, play_game_with, spectacle_fuzz_setup};

use crate::common::seeds::{for_each_seed, fuzz_games};

#[test]
fn bot_games_with_spectacle_afterlife_and_horsemanship_keep_every_invariant_and_use_each_part() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(16);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome = play_game_with(
                spectacle_fuzz_setup(players, seed),
                seed,
                PlayOptions::default(),
            )
            .unwrap_or_else(|failure| panic!("{failure}"));
            let mut seen = seen.lock().unwrap();
            // The seats that lost life in the turn so far.
            let mut lost: Vec<usize> = Vec::new();
            for event in &outcome.events {
                match event {
                    Event::StepBegan { step: Step::Untap } => lost.clear(),
                    Event::LifeLost { player, amount } | Event::LifePaid { player, amount }
                        if *amount > 0 =>
                    {
                        lost.push(player.index());
                    }
                    Event::DamageDealt {
                        target: mtg_engine::combat::DamageTarget::Player(player),
                        amount,
                        ..
                    } if *amount > 0 => lost.push(player.index()),
                    Event::SpellCast {
                        player, choices, ..
                    } if choices.way == Some(CastWay::Spectacle) => {
                        assert!(
                            lost.iter().any(|seat| *seat != player.index()),
                            "a spell was cast for its spectacle cost with no opponent having lost life (seed {seed}, {players} seats)"
                        );
                        *seen.entry("spectacle cast").or_default() += 1;
                    }
                    Event::TokenCreated(token) if token.name == "Spirit" => {
                        *seen.entry("spirit").or_default() += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let seen = seen.into_inner().unwrap();
    for part in ["spectacle cast", "spirit"] {
        assert!(
            seen.get(part).copied().unwrap_or(0) > 0,
            "no game went through {part}: {seen:?}"
        );
    }
}
