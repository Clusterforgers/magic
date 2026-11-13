//! Goal: every card the r55-04 pack made automatic reads back as its printed text clause by clause, and a near miss of
//! each new piece does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Report, Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report_of(name: &str, text: &str) -> Report {
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

/// Prints the report of the cards named in `R5504_PROBE` (separated by `|`).
#[test]
#[ignore = "probe"]
fn probe() {
    for name in std::env::var("R5504_PROBE").unwrap_or_default().split('|') {
        if name.is_empty() {
            continue;
        }
        let report = report_of(name, &file(name));
        println!("== {name}: {:?}\n{:#?}\n{:#?}", report.verdict(), report.clauses, report.problems);
    }
}

#[rstest]
#[case("Conspiracy")]
#[case("Painter's Servant")]
#[case("Rootpath Purifier")]
#[case("Rukarumel, Biologist")]
#[case("Rebound")]
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
#[case::in_addition("Conspiracy", r#"[{"SetCreatureTypes":[]},"AddChosenSubtype"]"#, r#"["AddChosenSubtype"]"#)]
#[case::only_the_color("Painter's Servant", r#""AddChosenColor""#, r#""SetChosenColor""#)]
#[case::the_hand("Rootpath Purifier", r#"{"In":"Library"}"#, r#"{"In":"Hand"}"#)]
#[case::tokens_too("Rukarumel, Biologist", r#"{"Not":"Token"}"#, r#""Token""#)]
#[case::any_new_target("Rebound", r#""AnotherPlayer""#, r#""Another""#)]
#[case::any_single_target("Rebound", r#"{"TargetsPlayer":"EachPlayer"},"#, "")]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}

/// The clause of Celestial Dawn's colors: whether it reads back.
fn dawn_reads(text: &str) -> bool {
    let report = report_of("Celestial Dawn", text);
    format!("{:?}", report.clauses[1].state).starts_with("Reads")
}

#[test]
fn celestial_dawns_colors_read_back_and_another_color_does_not() {
    let name = "Celestial Dawn";
    assert!(dawn_reads(&file(name)));
    assert!(!dawn_reads(&altered(name, r#"{"SetColors":["White"]}"#, r#"{"SetColors":["Blue"]}"#)));
    assert!(!dawn_reads(&altered(name, r#"{"Not":{"Type":"Land"}}"#, r#"{"Type":"Land"}"#)));
}
