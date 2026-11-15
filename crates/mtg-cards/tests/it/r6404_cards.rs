//! Goal: every card the r64-04 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::check;
use mtg_cards::support::PrintedCard;

/// The imported card of one fixture, without loading the whole corpus.
fn r6404_card(name: &str) -> PrintedCard {
    let path = common::fixture_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(path).unwrap();
    let record = mtg_import::scryfall::cards(format!("[{text}]").as_bytes())
        .next()
        .unwrap()
        .unwrap();
    common::printed_of(&record)
}

/// Prints how the compiler reads the cards of `R6404_COMPILE` and how the check judges the rules files of
/// `R6404_PROBE` (names separated by `|`); a tool for writing rules files.
#[test]
fn r6404_probe() {
    if let Ok(list) = std::env::var("R6404_COMPILE") {
        for name in list.split('|') {
            let compiled = mtg_cards::compile::compile(&r6404_card(name));
            println!("== {name}");
            for clause in &compiled.clauses {
                println!("  clause read={} {}", clause.read, clause.text);
            }
            if let Some(card) = compiled.card {
                for ability in card.faces.iter().flat_map(|face| &face.abilities) {
                    println!("  {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
    }
    let Ok(list) = std::env::var("R6404_PROBE") else {
        return;
    };
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    for name in list.split('|') {
        let card = r6404_card(name);
        let names: BTreeSet<String> = std::iter::once(&card.name)
            .chain(card.faces.iter().map(|face| &face.name))
            .map(|name| name.to_lowercase())
            .collect();
        let text =
            std::fs::read_to_string(common::cards_dir().join(format!("{}.json", slug(name))))
                .unwrap();
        let report = check(&text, &card, &tokens, &names, &Caps::everything());
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
        let parsed: mtg_cards::card::Card = serde_json::from_str(&text).unwrap();
        for ability in parsed.faces.iter().flat_map(|face| &face.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}
