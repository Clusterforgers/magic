//! Goal: every card the r47-08 pack made automatic reads back as its printed text clause by clause, a near miss of
//! each new piece does not, and the validator refuses each new node outside the shapes the engine runs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

fn file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

/// A rules file of the corpus with one piece of its compact JSON replaced.
fn altered(name: &str, from: &str, to: &str) -> String {
    let compact: serde_json::Value = serde_json::from_str(&file(name)).unwrap();
    let compact = serde_json::to_string(&compact).unwrap();
    assert!(compact.contains(from), "{name} holds {from}");
    compact.replacen(from, to, 1)
}

#[rstest]
#[case("Eligeth, Crossroads Augur")]
#[case("Enhanced Surveillance")]
#[case("Primitive Etchings")]
#[case("Rowen")]
#[case("Keranos, God of Storms")]
#[case("Inquisitor Eisenhorn")]
fn reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::an_opponents_scry(
    "Eligeth, Crossroads Augur",
    r#"{"Scries":{}}"#,
    r#"{"Scries":{"who":"Opponents"}}"#
)]
#[case::three_more(
    "Enhanced Surveillance",
    r#"{"MayAddAmount":2}"#,
    r#"{"MayAddAmount":3}"#
)]
#[case::an_opponents_surveil(
    "Enhanced Surveillance",
    r#"{"Surveils":{}}"#,
    r#"{"Surveils":{"who":"Opponents"}}"#
)]
#[case::reveal_optional(
    "Primitive Etchings",
    r#"{"RevealFirstDraw":{}}"#,
    r#"{"RevealFirstDraw":{"may":true}}"#
)]
#[case::reveal_your_turns(
    "Primitive Etchings",
    r#"{"RevealFirstDraw":{}}"#,
    r#"{"RevealFirstDraw":{"your_turns":true}}"#
)]
#[case::reveal_a_land("Primitive Etchings", r#"{"Type":"Creature"}"#, r#"{"Type":"Land"}"#)]
#[case::any_land("Rowen", r#"{"Supertype":"Basic"},"#, "")]
#[case::reveal_every_turn("Keranos, God of Storms", r#"{"your_turns":true}"#, "{}")]
#[case::reveal_always("Inquisitor Eisenhorn", r#"{"may":true}"#, "{}")]
#[case::revealed_creature(
    "Inquisitor Eisenhorn",
    r#"{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}"#,
    r#"{"Type":"Creature"}"#
)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::scry_drawing_one(
    "Eligeth, Crossroads Augur",
    r#"{"Draw":{"count":"Triggering"}}"#,
    r#"{"Draw":{}}"#
)]
#[case::scry_with_more_cards(
    "Eligeth, Crossroads Augur",
    r#"{"Instead":[{"Draw":{"count":"Triggering"}}]}"#,
    r#"{"MayAddAmount":2}"#
)]
#[case::surveil_instead(
    "Enhanced Surveillance",
    r#"{"MayAddAmount":2}"#,
    r#"{"Instead":[{"Draw":{}}]}"#
)]
#[case::no_more_cards(
    "Enhanced Surveillance",
    r#"{"MayAddAmount":2}"#,
    r#"{"MayAddAmount":0}"#
)]
#[case::more_draws("Enhanced Surveillance", r#"{"Surveils":{}}"#, r#"{"Draw":{}}"#)]
fn the_validator_refuses_other_shapes(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_eq!(
        report.verdict(),
        Verdict::Invalid,
        "{name}: {to}: {:#?}",
        report.problems
    );
}
