//! Goal: every card the r47-02 pack made automatic reads back as its printed text clause by clause, and a near miss
//! of each new wording does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Report, Verdict, check};
use mtg_cards::support::PrintedCard;
use mtg_import::scryfall::cards;
use rstest::rstest;

fn r4702_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

/// The printed card of `name`: the corpus fixture, else the record in `R4702_FIX` (a directory of Scryfall objects).
fn r4702_printed(name: &str) -> PrintedCard {
    if let Some(card) = common::printed().into_iter().find(|card| card.name == name) {
        return card;
    }
    let dir =
        PathBuf::from(std::env::var("R4702_FIX").expect("the corpus or R4702_FIX has the card"));
    let text = std::fs::read_to_string(dir.join(format!("{}.json", slug(name)))).unwrap();
    let record = cards(format!("[{text}]").as_bytes())
        .next()
        .unwrap()
        .unwrap();
    common::printed_of(&record)
}

fn r4702_file(name: &str) -> String {
    let path = std::env::var("R4702_RULES").map_or_else(|_| common::cards_dir(), PathBuf::from);
    std::fs::read_to_string(path.join(format!("{}.json", slug(name)))).unwrap()
}

fn r4702_report(name: &str, text: &str) -> Report {
    let mut cards = common::printed();
    let card = r4702_printed(name);
    cards.push(card.clone());
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    check(
        text,
        &card,
        &tokens,
        &r4702_names(&cards),
        &Caps::everything(),
    )
}

/// Prints compiled clauses (`R4702_COMPILE`), the wordings of abilities (`R4702_JSON`) and the clause states of
/// rules files (`R4702_PROBE`), names separated by `|`; a tool for writing rules files.
#[test]
fn r4702_probe() {
    if let Ok(json) = std::env::var("R4702_JSON") {
        for line in json.split('\n').filter(|line| !line.trim().is_empty()) {
            let ability: Ability = serde_json::from_str(line).unwrap();
            println!("-- {line}");
            for text in mtg_cards::compile::written_variants(&ability)
                .iter()
                .take(4)
            {
                println!("  written: {text}");
            }
        }
    }
    if let Ok(list) = std::env::var("R4702_COMPILE") {
        for name in list.split('|') {
            let compiled = mtg_cards::compile::compile(&r4702_printed(name));
            println!("== {name}");
            for clause in &compiled.clauses {
                println!("  clause read={} {}", clause.read, clause.text);
            }
            if let Some(card) = compiled.card {
                for face in &card.faces {
                    for ability in &face.abilities {
                        println!("  {}", serde_json::to_string(ability).unwrap());
                    }
                }
            }
        }
    }
    let Ok(list) = std::env::var("R4702_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r4702_report(name, &r4702_file(name));
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
        let Some(card) = report.rules else {
            continue;
        };
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Council of the Absolute")]
#[case("Alhammarret, High Arbiter")]
#[case("Tamiyo, Collector of Tales")]
#[case("Oko, Lorwyn Liege // Oko, Shadowmoor Scion")]
#[case("Alpine Moon")]
fn r4702_reads_back_whole(#[case] name: &str) {
    let report = r4702_report(name, &r4702_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::any_name_for_noncreature(
    "Council of the Absolute",
    r#""Choose": "NoncreatureNonlandCardName""#,
    r#""Choose": "NonlandCardName""#
)]
#[case::their_spells_cheaper(
    "Council of the Absolute",
    r#"{ "ControlledBy": "You" }"#,
    r#"{ "ControlledBy": "Opponents" }"#
)]
#[case::no_reveal(
    "Alhammarret, High Arbiter",
    r#""Choose": "RevealedNonlandName""#,
    r#""Choose": "NonlandCardName""#
)]
#[case::sacrifice_only(
    "Tamiyo, Collector of Tales",
    r#""OpponentsCantMakeDiscardOrSacrifice""#,
    r#""OpponentsCantMakeSacrifice""#
)]
#[case::emblem_of_another_type(
    "Oko, Lorwyn Liege // Oko, Shadowmoor Scion",
    r#"{ "ChosenAs": "type" }"#,
    r#"{ "ChosenAs": "other" }"#
)]
#[case::their_lands_keep_types("Alpine Moon", r#"{ "SetLandTypes": [] },"#, "")]
#[case::any_land_name(
    "Alpine Moon",
    r#""Choose": "NonbasicLandCardName""#,
    r#""Choose": "CardName""#
)]
fn r4702_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r4702_file(name);
    assert!(file.contains(from), "{name} holds {from}");
    let report = r4702_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name} with {to}");
}
