//! Goal: every card the r111-01 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another color, filter, count or cost), replaced wherever it occurs, does not.
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
#[case("Sunglasses of Urza")]
#[case("Celestial Dawn")]
#[case("Drought")]
#[case("Brutal Suppression")]
#[case("Argentum Masticore")]
fn r111_01_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::blue_as_red("Sunglasses of Urza", r#""mana":"White""#, r#""mana":"Blue""#)]
#[case::as_green("Sunglasses of Urza", r#""as_color":"Red""#, r#""as_color":"Green""#)]
#[case::others_free("Celestial Dawn", r#","others_colorless":true"#, "")]
#[case::opponents("Sunglasses of Urza", r#""who":"You""#, r#""who":"Opponents""#)]
#[case::plains("Drought", r#"{"Subtype":"Swamp"}"#, r#"{"Subtype":"Plains"}"#)]
#[case::per_red("Drought", r#""per_symbol":"Black""#, r#""per_symbol":"Red""#)]
#[case::tokens_too("Brutal Suppression", r#",{"Not":"Token"}"#, "")]
#[case::creature(
    "Brutal Suppression",
    r#""sacrifice":[{"Type":"Land"}]"#,
    r#""sacrifice":[{"Type":"Creature"}]"#
)]
#[case::greater("Argentum Masticore", r#""op":"Le""#, r#""op":"Ge""#)]
#[case::own(
    "Argentum Masticore",
    r#"{"ControlledBy":"Opponents"}"#,
    r#"{"ControlledBy":"You"}"#
)]
fn r111_01_a_near_miss_does_not_read_back(
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
fn r111_01_probe() {
    let name = std::env::var("MTG_PROBE").unwrap();
    for card in common::printed().iter().filter(|card| card.name == name) {
        let compiled = mtg_cards::compile::compile(card);
        println!("{}", mtg_cards::load::canonical(&compiled.card));
        for clause in &compiled.clauses {
            println!("{} {:?} {}", clause.read, clause.kind, clause.text);
        }
    }
}
