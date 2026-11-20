//! Goal: every card the r110-01 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another kind, cost, timing, count or color), replaced wherever it occurs, does not.
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

/// The rules file of a corpus card as compact JSON.
fn compact(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    serde_json::to_string(&value).unwrap()
}

#[rstest]
#[case("Dream Devourer")]
#[case("Bohn, Beguiling Balladeer")]
#[case("Cosmos Charger")]
#[case("Peter Parker // Amazing Spider-Man")]
fn r110_01_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::any_card("Dream Devourer", r#"{"Not":{"Type":"Land"}},"#, "")]
#[case::with_foretell(
    "Dream Devourer",
    r#"{"Not":{"HasKeywordNamed":"Foretell"}}"#,
    r#"{"HasKeywordNamed":"Foretell"}"#
)]
#[case::reduced_more(
    "Bohn, Beguiling Balladeer",
    r#"{"OwnManaCostLess":2}"#,
    r#"{"OwnManaCostLess":3}"#
)]
#[case::graveyard(
    "Bohn, Beguiling Balladeer",
    r#"{"In":"Hand"}"#,
    r#"{"In":"Graveyard"}"#
)]
#[case::opponent_foretells("Dream Devourer", r#""who":"You""#, r#""who":"Opponents""#)]
#[case::toughness(
    "Dream Devourer",
    r#""power":2,"toughness":0"#,
    r#""power":0,"toughness":2"#
)]
#[case::more("Cosmos Charger", r#""change":"Reduce""#, r#""change":"Increase""#)]
#[case::any_kind(
    "Cosmos Charger",
    r#""activating":"R110Foretell""#,
    r#""activating":"Any""#
)]
#[case::own_turn(
    "Cosmos Charger",
    r#"{"InstantActivation":{"kind":"R110Foretell"}},"#,
    ""
)]
#[case::nonlegendary(
    "Peter Parker // Amazing Spider-Man",
    r#"{"Supertype":"Legendary"},"#,
    ""
)]
#[case::colorless(
    "Peter Parker // Amazing Spider-Man",
    r#"{"Not":"Colorless"}"#,
    r#""Colorless""#
)]
#[case::untapped("Peter Parker // Amazing Spider-Man", r#","Tapped"]"#, "]")]
fn r110_01_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let text = compact(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replace(from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// Prints the compiler's reading of the corpus card `MTG_PROBE` names (a tool for writing rules, run by hand).
#[test]
#[ignore = "a probe run by hand"]
fn r110_01_probe() {
    let name = std::env::var("MTG_PROBE").unwrap();
    for card in common::printed().iter().filter(|card| card.name == name) {
        let compiled = mtg_cards::compile::compile(card);
        println!("{}", mtg_cards::load::canonical(&compiled.card));
        for clause in &compiled.clauses {
            println!("{} {:?} {}", clause.read, clause.kind, clause.text);
        }
    }
}
