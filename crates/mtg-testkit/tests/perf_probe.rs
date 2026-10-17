//! Goal: temporary timing probe.

use mtg_testkit::{fuzz_setup, play_game_with, PlayOptions};
use std::time::Instant;

#[test]
fn probe() {
    for players in [2usize, 6] {
        for check in [false, true] {
            let start = Instant::now();
            let mut answers = 0;
            let mut events = 0;
            let mut turns = 0;
            for seed in 0..10 {
                let o = play_game_with(
                    fuzz_setup(players, seed),
                    seed,
                    PlayOptions { check_invariants: check, ..PlayOptions::default() },
                )
                .unwrap();
                answers += o.answers;
                events += o.events.len();
                turns += o.turns;
            }
            eprintln!(
                "players={players} check={check}: {:?} for 10 games, answers={answers} events={events} turns={turns}",
                start.elapsed()
            );
        }
    }
}
