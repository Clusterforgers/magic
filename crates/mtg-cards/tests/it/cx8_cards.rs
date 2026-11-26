//! Goal: every card of the cx8 pack reads back as its printed text: the checker accepts its rules file whole, so each
//! is `Verified` by proof, and the same rules with one part changed are not accepted as the card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use mtg_cards::load::slug;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::Caps;
use rstest::rstest;

/// The cards of the pack.
const CARDS: &[&str] = &[
    "Celestial Reunion",
    "Eye of Ojer Taq // Apex Observatory",
    "Dack Fayden, Helping Hand",
    "Massimo, the Magician",
    "Q, Stacking the Deck",
    "The Mimeoplasm",
];

/// The verdict summary of `text` checked as the rules of `name`.
fn summary(name: &str, text: &str) -> (String, String) {
    let library = common::Corpus::repo().library();
    let printed = common::printed();
    let names: BTreeSet<String> = printed
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    let card = printed
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("no fixture for {name}"));
    let report = check_exact(text, card, &library, &names, &Caps::everything());
    let accepted = report.verdict() == Verdict::Accepted;
    (
        if accepted {
            report.summary()
        } else {
            "NOT ACCEPTED".to_owned()
        },
        report.render(true),
    )
}

fn rules_of(name: &str) -> String {
    std::fs::read_to_string(common::cards_dir().join(format!("{}.json", slug(name)))).unwrap()
}

#[test]
fn every_cx8_card_reads_back_as_its_text() {
    let mut failed = Vec::new();
    for name in CARDS {
        let (summary, render) = summary(name, &rules_of(name));
        if summary != "ACCEPTED Verified" || std::env::var("CX8_SHOW").is_ok() {
            failed.push(format!("{summary}\n{render}"));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}

/// One part of a card's rules changed: the checker reads it as other words.
#[rstest]
#[case::reveal_counts_each_player("Dack Fayden, Helping Hand", r#""Opponents""#, r#""EachPlayer""#)]
#[case::goaded_only_until_your_next_turn(
    "Dack Fayden, Helping Hand",
    r#""Forever""#,
    r#""UntilYourNextTurn""#
)]
#[case::behold_one("Celestial Reunion", r#""count": 2"#, r#""count": 3"#)]
#[case::search_greater("Celestial Reunion", r#""op": "Le""#, r#""op": "Ge""#)]
#[case::copy_paid_for("Massimo, the Magician", r#""free": true"#, r#""free": false"#)]
#[case::mana_value_two("Massimo, the Magician", r#""to": 1"#, r#""to": 2"#)]
#[case::any_time("Q, Stacking the Deck", r#""condition": "YourTurn","#, "")]
#[case::craft_any_two(
    "Eye of Ojer Taq // Apex Observatory",
    r#", "alike": "R702ShareCardType""#,
    ""
)]
#[case::until_land(
    "Q, Stacking the Deck",
    r#"{ "Not": { "Type": "Land" } }"#,
    r#"{ "Type": "Land" }"#
)]
fn a_changed_part_is_not_the_card(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let rules = rules_of(name);
    assert!(rules.contains(from), "{from}");
    let changed = rules.replacen(from, to, 1);
    assert_eq!(summary(name, &changed).0, "NOT ACCEPTED", "{changed}");
}
