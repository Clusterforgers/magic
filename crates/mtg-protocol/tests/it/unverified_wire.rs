//! Goal: the Unverified tier has its own spelling on the wire: the support word `unverified`, a count of its copies that a check stored before the tier reads as zero, an object flag every seat that sees the face reads (left out of the definition JSON, so no deck fingerprint moves), and TypeScript bindings that carry them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::generated;
use crate::common::table::Table;
use mtg_cards::CardDefinition;
use mtg_cards::deck::Counts;
use mtg_cards::support::Support;
use mtg_protocol::ObjectView;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Scenario, ready};
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
fn the_bindings_name_the_unverified_word_count_and_flag() {
    assert!(generated()["Support.ts"].contains("\"unverified\""));
    assert!(generated()["Counts.ts"].contains("unverified"));
    assert!(generated()["ObjectView.ts"].contains("unverified?: boolean"));
}

/// A 2/2 whose rules are a draft, unverified while `unread`.
fn draft_creature(name: &str, unread: bool) -> Arc<CardDefinition> {
    let mut definition = (*free_creature(name, 2, 2)).clone();
    definition.read = true;
    definition.unverified = mtg_cards::Shown(unread);
    Arc::new(definition)
}

#[rstest]
fn an_unverified_card_says_so_to_every_seat_that_sees_it_and_a_read_one_does_not(
    #[values(2, 3, 4)] seat_count: usize,
) {
    let mut scenario = Scenario::new(seat_count)
        .hand(0, [draft_creature("Hidden draft", true)])
        .library(0, vec![forest(); 4])
        .battlefield(
            0,
            [
                ready(draft_creature("Unread draft", true)),
                ready(draft_creature("Read draft", false)),
            ],
        );
    for other in 1..seat_count {
        scenario = scenario.library(other, vec![forest(); 4]);
    }
    let table = Table::new(scenario);
    for seat in 0..seat_count {
        let named = |name: &str| table.views[seat].objects.iter().find(|o| o.name == name);
        let unread = named("Unread draft").unwrap();
        assert!(unread.read && unread.unverified, "seat {seat}");
        let json = serde_json::to_value(unread).unwrap();
        assert_eq!(json["unverified"], json!(true));
        assert_eq!(&serde_json::from_value::<ObjectView>(json).unwrap(), unread);
        let read = named("Read draft").unwrap();
        assert!(read.read && !read.unverified, "seat {seat}");
        assert!(
            serde_json::to_value(read)
                .unwrap()
                .get("unverified")
                .is_none(),
            "left out while false"
        );
        let hidden = table.views[seat]
            .objects
            .iter()
            .any(|o| o.name.is_empty() && o.unverified);
        assert!(!hidden, "a hidden object says nothing of its card");
    }
}

#[test]
fn the_unverified_flag_stays_out_of_the_definition_json_so_no_deck_fingerprint_moves() {
    let unread = draft_creature("Unread draft", true);
    let read = draft_creature("Unread draft", false);
    assert_eq!(
        serde_json::to_value(&*unread).unwrap(),
        serde_json::to_value(&*read).unwrap()
    );
}
