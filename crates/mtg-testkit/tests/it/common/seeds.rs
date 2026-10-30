//! Goal: sizes a fuzz campaign (`MTG_FUZZ_GAMES`) and plays its seeds in parallel, so one campaign takes the time of its longest game instead of the sum of its games.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

/// Games of a campaign: `MTG_FUZZ_GAMES` when set (`MTG_FUZZ_GAMES=150` for a long run), else `default`.
pub fn fuzz_games(default: u64) -> u64 {
    env_games().unwrap_or(default)
}

/// Games of a campaign whose long run is heavier: `MTG_FUZZ_GAMES / divisor` (at least 1) when set, else `default`.
pub fn fuzz_games_scaled(default: u64, divisor: u64) -> u64 {
    env_games().map_or(default, |n| (n / divisor).max(1))
}

fn env_games() -> Option<u64> {
    std::env::var("MTG_FUZZ_GAMES").ok()?.parse().ok()
}

/// Threads of one campaign: `MTG_SEED_WORKERS`, else one when `RUST_TEST_THREADS` is set (the test threads already
/// fill the cores a run was given, as under `scripts/check.sh`), else one per core (a lone campaign uses the machine).
fn workers() -> usize {
    let var = |name: &str| std::env::var(name).ok()?.parse::<usize>().ok();
    var("MTG_SEED_WORKERS")
        .or_else(|| var("RUST_TEST_THREADS").map(|_| 1))
        .unwrap_or_else(|| thread::available_parallelism().map_or(4, usize::from))
        .max(1)
}

/// Runs `body` once for every seed in `seeds`, shifted by `MTG_FUZZ_FROM` when set, on `workers()` threads. Games do
/// not depend on each other, so campaign totals gathered behind a `Mutex` do not depend on the order. A seed that
/// panics stops the campaign, and the panic of the lowest failing seed is raised again with its own message.
pub fn for_each_seed(seeds: Range<u64>, body: impl Fn(u64) + Sync) {
    let from = std::env::var("MTG_FUZZ_FROM")
        .ok()
        .and_then(|from| from.parse::<u64>().ok())
        .unwrap_or(0);
    let seeds = seeds.start + from..seeds.end + from;
    let next = AtomicU64::new(seeds.start);
    let failures: Mutex<Vec<(u64, Box<dyn Any + Send>)>> = Mutex::new(Vec::new());
    let workers =
        workers().min(usize::try_from(seeds.end.saturating_sub(seeds.start)).unwrap_or(usize::MAX));
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    if !failures.lock().unwrap().is_empty() {
                        break;
                    }
                    let seed = next.fetch_add(1, Ordering::Relaxed);
                    if seed >= seeds.end {
                        break;
                    }
                    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| body(seed))) {
                        failures.lock().unwrap().push((seed, payload));
                    }
                }
            });
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort_by_key(|(seed, _)| *seed);
    if let Some((_, payload)) = failures.into_iter().next() {
        resume_unwind(payload);
    }
}
