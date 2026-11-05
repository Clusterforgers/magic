//! Goal: scratch measurement of the turn log in bot games.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::events::Event;
use mtg_testkit::{commander_fuzz_setup, play_game};

#[test]
fn measure_log() {
    for players in [2usize, 4] {
        let (mut max, mut sum, mut turns, mut total_events) = (0usize, 0usize, 0usize, 0usize);
        for seed in 0..6u64 {
            let setup = commander_fuzz_setup(players, seed).unwrap();
            let outcome = play_game(setup, seed).unwrap();
            let mut now = 0usize;
            for event in &outcome.events {
                total_events += 1;
                match event {
                    Event::TurnBegan { .. } => {
                        max = max.max(now);
                        sum += now;
                        turns += 1;
                        now = 0;
                    }
                    Event::DamageDealt { amount, .. } if *amount > 0 => now += 1,
                    Event::AttackersDeclared { attackers, .. } => now += attackers.len(),
                    Event::BlockersDeclared { blocks, .. } => now += blocks.len(),
                    Event::AbilityPutOnStack(put) if put.cause.is_none() => now += 1,
                    Event::CountersChanged(c) if c.delta > 0 => now += 1,
                    _ => {}
                }
            }
        }
        eprintln!(
            "LOGSIZE players={players} turns={turns} max_records_per_turn={max} mean={:.1} events={total_events}",
            sum as f64 / turns.max(1) as f64
        );
    }
}
