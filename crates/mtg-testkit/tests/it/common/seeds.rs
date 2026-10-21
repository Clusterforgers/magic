//! Goal: plays the seeds of a fuzz campaign on every core, so one campaign takes the time of its longest game instead of the sum of its games.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

/// Runs `body` once for every seed in `seeds` on one thread per core. Games do not depend on each other, so campaign
/// totals gathered behind a `Mutex` do not depend on the order. A seed that panics stops the campaign, and the panic of
/// the lowest failing seed is raised again with its own message.
pub fn for_each_seed(seeds: Range<u64>, body: impl Fn(u64) + Sync) {
    let next = AtomicU64::new(seeds.start);
    let failures: Mutex<Vec<(u64, Box<dyn Any + Send>)>> = Mutex::new(Vec::new());
    let workers = thread::available_parallelism().map_or(4, usize::from);
    let workers =
        workers.min(usize::try_from(seeds.end.saturating_sub(seeds.start)).unwrap_or(usize::MAX));
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
