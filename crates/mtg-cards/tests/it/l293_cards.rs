//! Goal: every card the l29-3 pack made fully automatic reads back as its printed text clause by clause, and each card
//! it left partial keeps by hand only the clauses it names.
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

/// Prints the clause states of the cards named in `L293_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn probe() {
    let Ok(list) = std::env::var("L293_PROBE") else {
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

/// Writes the compiled rules file of each Scryfall record in the directory `L293_COMPILE` to the directory `L293_OUT`;
/// a tool for starting a rules file from what the compiler reads.
#[test]
fn compiled() {
    let (Ok(input), Ok(output)) = (std::env::var("L293_COMPILE"), std::env::var("L293_OUT")) else {
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

#[rstest]
#[case("Entangling Trap")]
#[case("Rebellion of the Flamekin")]
#[case("Rielle, the Everwise")]
#[case("Veronica, Dissident Scribe")]
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
#[case::won_twice("Entangling Trap", r#""right":1"#, r#""right":2"#)]
#[case::lost("Entangling Trap", r#""op":"Ge""#, r#""op":"Lt""#)]
#[case::opponents_clash("Rebellion of the Flamekin", r#""who":"You""#, r#""who":"Opponents""#)]
#[case::won_clash_only(
    "Rebellion of the Flamekin",
    r#"{"Did":{"action":"Clashed","who":"You"}}"#,
    r#"{"W15cClashWon":{"who":"You"}}"#
)]
#[case::second_time("Rielle, the Everwise", r#""nth":1"#, r#""nth":2"#)]
#[case::every_discard(
    "Rielle, the Everwise",
    r#"{"Nth":{"event":{"Batch":{"Discarded":{"by":"You"}}},"nth":1}}"#,
    r#"{"Batch":{"Discarded":{"by":"You"}}}"#
)]
#[case::land_cards(
    "Veronica, Dissident Scribe",
    r#"{"Not":{"Type":"Land"}}"#,
    r#"{"Type":"Land"}"#
)]
#[case::any_card(
    "Veronica, Dissident Scribe",
    r#","what":[{"Not":{"Type":"Land"}}]"#,
    ""
)]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
