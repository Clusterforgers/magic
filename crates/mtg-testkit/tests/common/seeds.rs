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

/// Runs `body` once for every seed in `seeds`, shifted by `MTG_FUZZ_FROM` when set, on `workers()` threads; with
/// `MTG_FUZZ_SEED` set it plays that one seed alone. Games do not depend on each other, so campaign totals gathered
/// behind a `Mutex` do not depend on the order. A seed that panics stops the campaign, the lowest failing seed is
/// printed with the command that plays it again, and its panic is raised again with its own message.
pub fn for_each_seed(seeds: Range<u64>, body: impl Fn(u64) + Sync) {
    let var = |name: &str| std::env::var(name).ok()?.parse::<u64>().ok();
    let seeds = campaign_seeds(
        seeds,
        var("MTG_FUZZ_SEED"),
        var("MTG_FUZZ_FROM").unwrap_or(0),
    );
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
    if let Some((seed, payload)) = failures.into_iter().next() {
        eprintln!("{}", rerun_hint(seed));
        resume_unwind(payload);
    }
}

/// The seeds a campaign plays: `only` alone when given (`MTG_FUZZ_SEED`), else `seeds` shifted by `from`.
pub fn campaign_seeds(seeds: Range<u64>, only: Option<u64>, from: u64) -> Range<u64> {
    match only {
        Some(seed) => seed..seed.saturating_add(1),
        None => seeds.start.saturating_add(from)..seeds.end.saturating_add(from),
    }
}

/// How to play a failing seed again: the seeds of a campaign are fixed, so the test by its name replays it, and
/// `MTG_FUZZ_SEED` plays that seed alone (the campaign's totals then count that one game).
pub fn rerun_hint(seed: u64) -> String {
    let current = thread::current();
    let test = current
        .name()
        .filter(|name| *name != "main")
        .unwrap_or("<test path>");
    format!(
        "seed {seed} failed; replay: cargo test -p mtg-testkit --test <family> -- --exact {test} \
         (MTG_FUZZ_SEED={seed} plays this seed alone)"
    )
}
