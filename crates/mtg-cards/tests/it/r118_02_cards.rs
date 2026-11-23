//! Goal: every card the r118-02 pack made fully automatic (and Gideon, Champion of Justice) reads back as its printed
//! text clause by clause, the compiler reads it whole, and a near miss of each (another number, filter, kind or holder of
//! the counters) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::compile;
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

fn printed(name: &str) -> PrintedCard {
    common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"))
}

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    check(
        text,
        &printed(name),
        &tokens,
        &names(&cards),
        &Caps::everything(),
    )
}

/// The rules file of a corpus card as compact JSON.
fn compact(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    serde_json::to_string(&value).unwrap()
}

#[rstest]
#[case("Frankenstein's Monster")]
#[case("Gideon, Champion of Justice")]
#[case("Haste Magic")]
#[case("Inti, Seneschal of the Sun")]
#[case("Nahiri's Sacrifice")]
#[case("Questing Druid // Seek the Beast")]
#[case("Riveteers Charm")]
#[case("Volatile Stormdrake")]
#[case("Wiccan, Young Avenger")]
#[case("Yasmin Khan")]
fn r118_02_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case("Frankenstein's Monster")]
#[case("Gideon, Champion of Justice")]
#[case("Inti, Seneschal of the Sun")]
#[case("Riveteers Charm")]
#[case("Volatile Stormdrake")]
#[case("Wiccan, Young Avenger")]
fn r118_02_the_compiler_reads_whole(#[case] name: &str) {
    let compiled = compile(&printed(name));
    assert_eq!(
        compiled.card.map(|card| card.impl_status),
        Some(ImplStatus::Full),
        "{name}"
    );
}

/// The cost of Nahiri's Sacrifice reads by the X announced; its divided damage is the hand-written spell of Fire Covenant.
#[test]
fn r118_02_a_sacrifice_by_x_reads_as_a_required_cost_by_x() {
    let compiled = compile(&printed("Nahiri's Sacrifice"));
    let card = compiled.card.unwrap();
    let text = serde_json::to_string(&card.faces[0].abilities[0]).unwrap();
    assert!(
        text.contains(r#""stat":"ManaValue","op":"Eq","to":"X""#)
            && text.contains(r#""required":true"#),
        "{text}"
    );
}

#[rstest]
#[case(
    "Inti, Seneschal of the Sun",
    r#""duration":"UntilYourNextEndStep""#,
    r#""duration":"UntilEndOfYourNextTurn""#
)]
#[case("Nahiri's Sacrifice", r#""op":"Eq""#, r#""op":"Le""#)]
#[case(
    "Volatile Stormdrake",
    r#"{"Ability":"Triggered"}"#,
    r#"{"Ability":"Activated"}"#
)]
#[case("Volatile Stormdrake", r#""count":4"#, r#""count":3"#)]
#[case(
    "Frankenstein's Monster",
    r#"{"Other":"+0/+2"}"#,
    r#"{"Other":"+0/+3"}"#
)]
#[case("Frankenstein's Monster", r#""Creature""#, r#""Artifact""#)]
#[case(
    "Gideon, Champion of Justice",
    r#""counter":"loyalty","on":"This"}"#,
    r#""counter":"loyalty","on":{"Target":0}}"#
)]
#[case(
    "Gideon, Champion of Justice",
    r#""what":"Opponent""#,
    r#""what":"Player""#
)]
fn r118_02_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let text = compact(name);
    assert!(text.contains(from), "{name}: {text}");
    let report = report_of(name, &text.replacen(from, to, 1));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to} {:#?}",
        report.clauses
    );
}
