//! Goal: every card the r47-05 pack made automatic reads back as its printed text clause by clause, and a near miss of
//! each new piece does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, canonical, slug};
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

/// The printed card of a Scryfall record file outside the corpus.
fn printed_at(path: &str) -> PrintedCard {
    let text = std::fs::read_to_string(path).unwrap();
    let mut found: Vec<_> = mtg_import::scryfall::cards(format!("[{text}]").as_bytes()).collect();
    common::printed_of(&found.remove(0).unwrap())
}

/// Prints what the compiler reads from each Scryfall record named in `R4705_COMPILE` (paths separated by `|`), with
/// the check of that reading; a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R4705_COMPILE") else {
        return;
    };
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    for path in list.split('|') {
        let card = printed_at(path);
        let compiled = mtg_cards::compile::compile(&card);
        println!("== {}", card.name);
        for clause in &compiled.clauses {
            println!("  {:?} read={} {}", clause.kind, clause.read, clause.text);
        }
        let Some(rules) = compiled.card else {
            continue;
        };
        let json = canonical(&rules);
        if std::env::var("R4705_JSON").is_ok() {
            println!("{json}");
        }
        let report = check(&json, &card, &tokens, &names(&cards), &Caps::everything());
        println!("  verdict {:?}", report.verdict());
        for clause in &report.clauses {
            println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
        }
    }
}

/// Prints the check of the rules file `R4705_RULES` against the Scryfall record `R4705_RECORD`.
#[test]
fn probe_rules() {
    let (Ok(rules), Ok(record)) = (std::env::var("R4705_RULES"), std::env::var("R4705_RECORD"))
    else {
        return;
    };
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = printed_at(&record);
    let text = std::fs::read_to_string(rules).unwrap();
    let report = check(&text, &card, &tokens, &names(&cards), &Caps::everything());
    println!("  verdict {:?}", report.verdict());
    for problem in &report.problems {
        println!("  problem: {problem:?}");
    }
    for clause in &report.clauses {
        println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
    }
    for stray in &report.strays {
        println!("  stray: {stray:?}");
    }
}

/// Prints every wording the writers give the ability JSON in `R4705_WRITE`.
#[test]
fn written() {
    let Ok(json) = std::env::var("R4705_WRITE") else {
        return;
    };
    let ability: mtg_cards::ability::Ability = serde_json::from_str(&json).unwrap();
    for text in mtg_cards::compile::written_variants(&ability) {
        println!("  W: {text}");
    }
}

/// A rules file of the corpus with one piece of its JSON replaced.
fn altered(name: &str, from: &str, to: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let compact: serde_json::Value = serde_json::from_str(&text).unwrap();
    let compact = serde_json::to_string(&compact).unwrap();
    assert!(compact.contains(from), "{name} holds {from}");
    compact.replacen(from, to, 1)
}

#[rstest]
#[case("Gaea's Liege")]
#[case("Thromok the Insatiable")]
#[case("The Serpent Society")]
#[case("Imperiosaur")]
#[case("Myr Superion")]
#[case("Hand of Vecna")]
#[case("Dralnu, Lich Lord")]
#[case("Teferi's Talent")]
#[case("Carth the Lion")]
#[case("Rowan's Talent")]
fn reads_back_whole(#[case] name: &str) {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let report = report_of(name, &std::fs::read_to_string(&path).unwrap());
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::liege_attacking_you(
    "Gaea's Liege",
    r#"{"ControlledBy":"Defending"}"#,
    r#"{"ControlledBy":"You"}"#
)]
#[case::liege_always(
    "Gaea's Liege",
    r#","condition":{"Not":{"Compare":{"left":{"Count":["This","Attacking"]},"op":"Ge","right":1}}}"#,
    ""
)]
#[case::devour_one("Thromok the Insatiable", r#""counters":0"#, r#""counters":1"#)]
#[case::devour_artifacts(
    "Thromok the Insatiable",
    r#"{"Type":"Creature"}"#,
    r#"{"Type":"Artifact"}"#
)]
#[case::four_poison("The Serpent Society", r#"{"GetPoison":5}"#, r#"{"GetPoison":4}"#)]
#[case::five_life("The Serpent Society", r#"{"GetPoison":5}"#, r#"{"PayLife":5}"#)]
#[case::any_land("Imperiosaur", r#""supertypes":["Basic"],"#, "")]
#[case::basic_creatures(
    "Myr Superion",
    r#""types":["Creature"]"#,
    r#""types":["Creature"],"supertypes":["Basic"]"#
)]
#[case::artifacts("Myr Superion", r#""types":["Creature"]"#, r#""types":["Artifact"]"#)]
#[case::life_for_each_land(
    "Hand of Vecna",
    r#"{"PayLife":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}"#,
    r#"{"PayLife":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}"#
)]
#[case::sacrifice_creatures(
    "Dralnu, Lich Lord",
    r#""filter":[]"#,
    r#""filter":[{"Type":"Creature"}]"#
)]
#[case::flashback_for_good("Dralnu, Lich Lord", r#","duration":"UntilEndOfTurn""#, "")]
#[case::any_graveyard("Dralnu, Lich Lord", r#",{"OwnedBy":"You"}"#, "")]
#[case::loyalty_twice(
    "Teferi's Talent",
    r#"{"LoyaltyAnyTime":"You"}"#,
    r#"{"LoyaltyTwice":"You"}"#
)]
#[case::minus_ten("Teferi's Talent", r#"{"Minus":12}"#, r#"{"Minus":10}"#)]
#[case::plus_two("Carth the Lion", r#""plus":1"#, r#""plus":2"#)]
#[case::opponents_activate("Carth the Lion", r#""who":"You""#, r#""who":"Opponents""#)]
#[case::any_loyalty_ability("Rowan's Talent", r#""what":[{"Is":"Attached"}],"#, "")]
#[case::same_targets("Rowan's Talent", r#""new_targets":true,"#, "")]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}
