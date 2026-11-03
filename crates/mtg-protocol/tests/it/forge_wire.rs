//! Goal: the Forge tier has its own spelling on the wire: the support word `forge`, a count of its copies that a check stored before the tier reads as zero, and TypeScript bindings that carry both.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::generated;
use mtg_cards::deck::Counts;
use mtg_cards::support::Support;
use serde_json::json;

#[test]
fn the_forge_support_word_round_trips_and_is_spelled_forge() {
    assert_eq!(
        serde_json::to_value(Support::Forge).unwrap(),
        json!("forge")
    );
    let back: Support = serde_json::from_value(json!("forge")).unwrap();
    assert_eq!(back, Support::Forge);
}

#[test]
fn counts_round_trip_with_forge_and_a_check_stored_before_the_tier_has_none() {
    let counts = Counts {
        total: 10,
        verified: 6,
        read: 1,
        forge: 3,
        partial: 0,
        manual: 0,
        stale: 0,
        unplayable: 0,
    };
    let value = serde_json::to_value(counts).unwrap();
    assert_eq!(value["forge"], 3);
    let back: Counts = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(back, counts);

    let mut old = value;
    old.as_object_mut().unwrap().remove("forge");
    let back: Counts = serde_json::from_value(old).unwrap();
    assert_eq!(back.forge, 0);
}

#[test]
fn the_bindings_name_the_forge_word_and_the_forge_count() {
    assert!(generated()["Support.ts"].contains("\"forge\""));
    assert!(generated()["Counts.ts"].contains("forge"));
}
