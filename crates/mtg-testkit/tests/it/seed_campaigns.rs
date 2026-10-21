//! Goal: a campaign played on every core covers each seed exactly once whatever the range, stops after a failure, and reports the lowest failing seed's own panic, as a sequential loop would.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rstest::rstest;

use crate::common::seeds::for_each_seed;

#[rstest]
#[case::empty(0..0)]
#[case::one(7..8)]
#[case::fewer_seeds_than_cores(0..3)]
#[case::offset(100..357)]
#[case::many(0..5000)]
fn every_seed_of_the_range_is_played_exactly_once(#[case] seeds: std::ops::Range<u64>) {
    let played = Mutex::new(BTreeMap::<u64, u32>::new());
    for_each_seed(seeds.clone(), |seed| {
        *played.lock().unwrap().entry(seed).or_default() += 1;
    });
    let played = played.into_inner().unwrap();
    let wanted: BTreeMap<u64, u32> = seeds.map(|seed| (seed, 1)).collect();
    assert_eq!(played, wanted);
}

#[rstest]
#[case::the_only_failure(&[41])]
#[case::two_failures(&[17, 5])]
#[case::the_first_seed(&[0, 63])]
#[case::neighbours(&[30, 31, 32])]
fn the_panic_of_the_lowest_failing_seed_is_raised_again_with_its_message(#[case] failing: &[u64]) {
    let lowest = *failing.iter().min().unwrap();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        for_each_seed(0..64, |seed| {
            // Later seeds finish sooner, so a higher failing seed is usually reported first by the clock.
            std::thread::sleep(Duration::from_micros(64 - seed));
            assert!(!failing.contains(&seed), "seed {seed} failed");
        });
    }));
    let payload = outcome.unwrap_err();
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_default();
    assert!(
        message.contains(&format!("seed {lowest} failed")),
        "{message}"
    );
}

#[test]
fn a_failure_stops_the_campaign_long_before_its_last_seed() {
    let played = AtomicU64::new(0);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        for_each_seed(0..1_000_000, |seed| {
            played.fetch_add(1, Ordering::Relaxed);
            assert!(seed != 0, "seed 0 failed");
            std::thread::sleep(Duration::from_millis(1));
        });
    }));
    assert!(outcome.is_err());
    assert!(played.load(Ordering::Relaxed) < 100_000);
}
