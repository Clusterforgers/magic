//! Goal: every card the r117-02 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another number, filter or trigger) does not.
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
#[case("Alexander, Ship's Fool")]
#[case("Ambassador K'Ehleyr")]
#[case("Dathon, His Heart Open")]
#[case("First Contact")]
#[case("Grilka, Widowed Warrior")]
#[case("Hugh, First of Many")]
#[case("Merciful Mandate")]
#[case("Olinda the Oblivious")]
#[case("Titania, Rugged Rumbler")]
#[case("Tuvok, Chief Tactical Officer")]
#[case("Worf, Intelligence Contractor")]
fn r117_02_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case(
    "Alexander, Ship's Fool",
    r#""from":"Opponents""#,
    r#""from":"EachPlayer""#
)]
#[case("Alexander, Ship's Fool", r#""stat":"ManaValue""#, r#""stat":"Power""#)]
#[case(
    "Ambassador K'Ehleyr",
    r#"[{"Not":{"Type":"Land"}}]"#,
    r#"[{"Type":"Land"}]"#
)]
#[case(
    "Dathon, His Heart Open",
    r#"{"R117AnotherOfKind":{"on":{"Target":0}}}"#,
    r#"{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}"#
)]
#[case(
    "Dathon, His Heart Open",
    r#"{"Subtype":"Saga"}"#,
    r#"{"Subtype":"Shrine"}"#
)]
#[case(
    "First Contact",
    r#""condition":"R117FirstSpellThisGame""#,
    r#""condition":"Kicked""#
)]
#[case("First Contact", r#""count":4"#, r#""count":3"#)]
#[case(
    "Hugh, First of Many",
    r#""Replicate":"{2}{U}""#,
    r#""Replicate":"{2}{B}""#
)]
#[case(
    "Hugh, First of Many",
    r#"{"Type":"Artifact"}"#,
    r#"{"Type":"Creature"}"#
)]
#[case("Tuvok, Chief Tactical Officer", r#""max":2"#, r#""max":3"#)]
#[case(
    "Tuvok, Chief Tactical Officer",
    r#""Step":"DeclareAttackers""#,
    r#""Step":"DeclareBlockers""#
)]
#[case("Grilka, Widowed Warrior", r#""tapped":true"#, r#""tapped":false"#)]
#[case("Grilka, Widowed Warrior", r#"{"AddKeyword":"Trample"},"#, "")]
#[case(
    "Olinda the Oblivious",
    r#"{"Not":{"HasCounter":{"Other":"odor"}}}"#,
    r#"{"Type":"Creature"}"#
)]
#[case(
    "Olinda the Oblivious",
    r#""duration":{"WhileHasCounter":{"Other":"odor"}}"#,
    r#""duration":"UntilEndOfTurn""#
)]
#[case("Worf, Intelligence Contractor", r#""to":3"#, r#""to":2"#)]
#[case("Worf, Intelligence Contractor", r#"{"AddSubtype":"Warrior"},"#, "")]
#[case("Merciful Mandate", r#"["R117EndCombat"]"#, r#"["EndTurn"]"#)]
#[case(
    "Merciful Mandate",
    r#""condition":"InCombat""#,
    r#""condition":"AfterCombat""#
)]
#[case(
    "Titania, Rugged Rumbler",
    r#"{"Ward":[{"OneOf":[[{"Discard":{}}],"{2}"]}]}"#,
    r#"{"Ward":[{"OneOf":[[{"Discard":{}}],"{3}"]}]}"#
)]
fn r117_02_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let text = compact(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replace(from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
