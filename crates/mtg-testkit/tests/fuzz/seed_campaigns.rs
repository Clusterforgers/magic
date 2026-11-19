//! Goal: a campaign played on every core covers each seed exactly once whatever the range, stops after a failure, and reports the lowest failing seed's own panic, as a sequential loop would; `MTG_FUZZ_SEED` narrows it to one seed, and a failure names the command that replays it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rstest::rstest;

use crate::common::seeds::{campaign_seeds, for_each_seed, rerun_hint};

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

#[rstest]
#[case::the_campaign(0..40, None, 0, 0..40)]
#[case::shifted(0..40, None, 1000, 1000..1040)]
#[case::one_seed_inside(0..40, Some(17), 0, 17..18)]
#[case::one_seed_outside(0..40, Some(91), 0, 91..92)]
#[case::one_seed_ignores_the_shift(0..40, Some(3), 500, 3..4)]
fn the_seeds_of_a_campaign_follow_the_environment(
    #[case] seeds: std::ops::Range<u64>,
    #[case] only: Option<u64>,
    #[case] from: u64,
    #[case] wanted: std::ops::Range<u64>,
) {
    assert_eq!(campaign_seeds(seeds, only, from), wanted);
}

#[rstest]
#[case(0)]
#[case(17)]
#[case(u64::MAX)]
fn a_failing_seed_names_the_command_that_replays_it(#[case] seed: u64) {
    let hint = rerun_hint(seed);
    assert!(hint.starts_with(&format!("seed {seed} failed;")), "{hint}");
    assert!(hint.contains("cargo test -p mtg-testkit --test"), "{hint}");
    assert!(hint.contains(&format!("MTG_FUZZ_SEED={seed} ")), "{hint}");
}
