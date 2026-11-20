//! Goal: every card the r112-01 pack made fully automatic reads back as its printed text clause by clause, and a near miss
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
#[case("Quicksilver Elemental")]
#[case("Jegantha, the Wellspring")]
#[case("Security Rhox")]
#[case("North Star")]
#[case("Jeweled Amulet")]
#[case("Emblazoned Golem")]
#[case("Night Soil")]
#[case("Protective Sphere")]
#[case("Brigid, Clachan's Heart // Brigid, Doun's Mind")]
fn r112_01_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::red("Quicksilver Elemental", r#""mana":"Blue""#, r#""mana":"Red""#)]
#[case::everything("Quicksilver Elemental", r#","r112_own_abilities":true"#, "")]
#[case::generic(
    "Jegantha, the Wellspring",
    r#"["R112NotGeneric"]"#,
    r#"["CostWithX"]"#
)]
#[case::clues("Security Rhox", r#""Treasure""#, r#""Clue""#)]
#[case::flashback("Security Rhox", r#""way":"Free""#, r#""way":"Flashback""#)]
#[case::any_color("North Star", r#""AnyType""#, r#""AnyColor""#)]
#[case::five("North Star", r#""{4}""#, r#""{5}""#)]
#[case::all_noted(
    "Jeweled Amulet",
    r#"{"R112Noted":{}}"#,
    r#"{"R112Noted":{"amount":true}}"#
)]
#[case::some_counters("Jeweled Amulet", r#""op":"Eq""#, r#""op":"Ge""#)]
#[case::no_note("Jeweled Amulet", r#","R112NoteManaSpent""#, "")]
#[case::white_x(
    "Emblazoned Golem",
    r#"["R112XOneEachColor"]"#,
    r#"[{"XColor":"White"}]"#
)]
#[case::two_counters("Emblazoned Golem", r#""count":"X""#, r#""count":2"#)]
#[case::any_graveyards(
    "Night Soil",
    r#"{"R112ExileFromOneGraveyard":{"count":2,"filter":[{"Type":"Creature"}]}}"#,
    r#"{"ExileFromGraveyard":{"any_player":true,"count":2,"filter":[{"Type":"Creature"}]}}"#
)]
#[case::any_source("Protective Sphere", r#","R112SharesColorWithSpent""#, "")]
#[case::next_only("Protective Sphere", r#""by":"Prevent""#, r#""by":"PreventNext""#)]
#[case::blue(
    "Brigid, Clachan's Heart // Brigid, Doun's Mind",
    r#"["Green","White"]"#,
    r#"["Green","Blue"]"#
)]
#[case::all_creatures(
    "Brigid, Clachan's Heart // Brigid, Doun's Mind",
    r#"},"Other",{"#,
    r#"},{"#
)]
fn r112_01_a_near_miss_does_not_read_back(
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
fn r112_01_probe() {
    let name = std::env::var("MTG_PROBE").unwrap();
    for card in common::printed().iter().filter(|card| card.name == name) {
        let compiled = mtg_cards::compile::compile(card);
        println!("{}", mtg_cards::load::canonical(&compiled.card));
        for clause in &compiled.clauses {
            println!("{} {:?} {}", clause.read, clause.kind, clause.text);
        }
    }
}
