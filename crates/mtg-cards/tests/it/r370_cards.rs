//! Goal: every card the r37-0 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss of its rules does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, canonical, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use mtg_import::scryfall::cards;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report(name: &str) -> mtg_cards::proposal::Report {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    report_of(name, &std::fs::read_to_string(&path).unwrap())
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

/// Prints the clause states of the cards named in `R370_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("R370_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = report(name);
        println!("== {name}: {:?}", report.verdict());
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
}

/// Writes the compiled rules file of each Scryfall record in the directory `R370_COMPILE` to the directory `R370_OUT`;
/// a tool for starting a rules file from what the compiler reads.
#[test]
fn compiled() {
    let (Ok(input), Ok(output)) = (std::env::var("R370_COMPILE"), std::env::var("R370_OUT")) else {
        return;
    };
    for entry in std::fs::read_dir(&input).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let Some(Ok(record)) = cards(format!("[{text}]").as_bytes()).next() else {
            continue;
        };
        let printed = common::printed_of(&record);
        let Some(card) = mtg_cards::compile::compile(&printed).card else {
            continue;
        };
        let out = std::path::Path::new(&output).join(path.file_name().unwrap());
        std::fs::write(out, canonical(&card)).unwrap();
    }
}

/// Checks each rules file of the directory `R370_CHECK` against the Scryfall record of the same file name in
/// `R370_FX` and prints its clause states; a tool for writing rules files of cards outside the corpus.
#[test]
fn check_outside() {
    let (Ok(rules), Ok(records)) = (std::env::var("R370_CHECK"), std::env::var("R370_FX")) else {
        return;
    };
    let corpus = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let mut entries: Vec<_> = std::fs::read_dir(&rules)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let record = std::path::Path::new(&records).join(path.file_name().unwrap());
        let text = std::fs::read_to_string(&record).unwrap();
        let Some(Ok(record)) = cards(format!("[{text}]").as_bytes()).next() else {
            continue;
        };
        let printed = common::printed_of(&record);
        let mut known = names(&corpus);
        known.insert(printed.name.to_lowercase());
        let report = check(
            &std::fs::read_to_string(&path).unwrap(),
            &printed,
            &tokens,
            &known,
            &Caps::everything(),
        );
        println!("== {}: {:?}", printed.name, report.verdict());
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
}

#[rstest]
#[case("Divine Deflection")]
#[case("Acolyte's Reward")]
#[case("Refraction Trap")]
#[case("Vengeful Archon")]
#[case("Restore Balance")]
#[case("Magus of the Balance")]
#[case("Portent Tracker")]
#[case("Etched Host Doombringer")]
#[case("Essence of the Wild")]
#[case("March from Velis Vel")]
#[case("Aang, Swift Savior // Aang and La, Ocean's Fury")]
#[case("Assimilation Aegis")]
#[case("Blade of Shared Souls")]
#[case("Black Hole")]
fn reads_back_whole(#[case] name: &str) {
    let report = report(name);
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
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
#[case::not_yours(
    "Divine Deflection",
    r#"{"Union":["You",{"Each":[{"ControlledBy":"You"}]}]}"#,
    r#""You""#
)]
#[case::devotion_to_blue(
    "Acolyte's Reward",
    r#"{"Devotion":["White"]}"#,
    r#"{"Devotion":["Blue"]}"#
)]
#[case::to_the_first_target("Acolyte's Reward", r#"{"Target":1}"#, r#"{"Target":0}"#)]
#[case::four_damage("Refraction Trap", r#""most":3"#, r#""most":4"#)]
#[case::to_a_creature(
    "Vengeful Archon",
    r#"{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}"#,
    r#""AnyTarget""#
)]
#[case::creatures_first("Restore Balance", r#"[{"Type":"Land"}]"#, r#"[{"Type":"Creature"}]"#)]
#[case::no_hand("Magus of the Balance", r#",{"Balance":{"what":"Hand"}}"#, "")]
#[case::you_protect(
    "Portent Tracker",
    r#"{"ProtectedBy":"Opponents"}"#,
    r#"{"ProtectedBy":"You"}"#
)]
#[case::two_counters("Etched Host Doombringer", r#""count":3"#, r#""count":2"#)]
#[case::any_land_type("March from Velis Vel", r#""NonbasicLandType""#, r#""LandType""#)]
#[case::no_haste(
    "March from Velis Vel",
    r#""AddKeyword":"Haste""#,
    r#""AddKeyword":"Flying""#
)]
#[case::may_copy("Essence of the Wild", r#""must":true"#, r#""must":false"#)]
#[case::opponents_creatures(
    "Essence of the Wild",
    r#"{"ControlledBy":"You"}"#,
    r#"{"ControlledBy":"Opponents"}"#
)]
#[case::three_mana(
    "Aang, Swift Savior // Aang and La, Ocean's Fury",
    r#""cost":"{2}""#,
    r#""cost":"{3}""#
)]
#[case::until_end_of_turn(
    "Assimilation Aegis",
    r#""WhileSourceAttached""#,
    r#""UntilEndOfTurn""#
)]
#[case::must_copy(
    "Blade of Shared Souls",
    r#"{"May":{"effects":[{"BecomeCopy""#,
    r#"{"Together":{"effects":[{"BecomeCopy""#
)]
#[case::scried("Black Hole", r#""AttractionsVisited""#, r#""Scried""#)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let changed = altered(name, from, to);
    let report = report_of(name, &changed);
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {from} -> {to}"
    );
}
