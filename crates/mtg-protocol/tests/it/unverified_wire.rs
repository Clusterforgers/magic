//! Goal: the Unverified tier has its own spelling on the wire: the support word `unverified`, a count of its copies that a check stored before the tier reads as zero, and TypeScript bindings that carry both.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_cards::deck::Counts;
use mtg_cards::support::Support;
use rstest::rstest;
use serde_json::json;

#[rstest]
#[case::unverified(Support::Unverified, "unverified")]
#[case::read(Support::Read, "read")]
#[case::forge(Support::Forge, "forge")]
fn each_tier_that_runs_whole_has_a_word_of_its_own(#[case] support: Support, #[case] word: &str) {
    assert_eq!(serde_json::to_value(support).unwrap(), json!(word));
    let back: Support = serde_json::from_value(json!(word)).unwrap();
    assert_eq!(back, support);
    assert!(support.runs_whole());
}

#[test]
fn counts_round_trip_with_unverified_and_a_check_stored_before_the_tier_has_none() {
    let counts = Counts {
        total: 10,
        verified: 2,
        read: 1,
        forge: 3,
        unverified: 4,
        ..Counts::default()
    };
    let value = serde_json::to_value(counts).unwrap();
    assert_eq!(value["unverified"], 4);
    let back: Counts = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(back, counts);

    let mut old = value;
    old.as_object_mut().unwrap().remove("unverified");
    let back: Counts = serde_json::from_value(old).unwrap();
    assert_eq!(back.unverified, 0);
}

#[test]
fn the_bindings_name_the_unverified_word_and_count() {
    assert!(generated()["Support.ts"].contains("\"unverified\""));
    assert!(generated()["Counts.ts"].contains("unverified"));
}
