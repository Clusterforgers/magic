//! Goal: a scratch probe that prints what the compiler reads and how the checker pairs a rules file (run by hand).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use crate::common;
use mtg_cards::Caps;
use mtg_cards::compile::compile;
use mtg_cards::compile::written_variants;
use mtg_cards::load::Library;
use mtg_import::scryfall::cards;

#[test]
#[ignore = "a probe run by hand"]
fn r5516_probe() {
    let fixture = std::env::var("R5516_FIXTURE").unwrap();
    let rules = std::env::var("R5516_RULES").ok();
    let text = std::fs::read_to_string(&fixture).unwrap();
    let record = cards(format!("[{text}]").as_bytes())
        .next()
        .unwrap()
        .unwrap();
    let printed = common::printed_of(&record);
    let compiled = compile(&printed);
    for face in &compiled
        .card
        .iter()
        .flat_map(|card| card.faces.clone())
        .collect::<Vec<_>>()
    {
        for ability in &face.abilities {
            println!("COMPILED {}", serde_json::to_string(ability).unwrap());
        }
    }
    let Some(rules) = rules else { return };
    let json = std::fs::read_to_string(rules).unwrap();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let names = common::printed()
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    let report = mtg_cards::proposal::check(&json, &printed, &tokens, &names, &Caps::everything());
    println!("VERDICT {:?}", report.verdict());
    println!("PROBLEMS {:#?}", report.problems);
    println!("STRAYS {:#?}", report.strays);
    for clause in &report.clauses {
        println!(
            "CLAUSE {:?} {:?}\n  {}",
            clause.kind, clause.state, clause.text
        );
    }
    if let Some(card) = &report.rules {
        for face in &card.faces {
            for ability in &face.abilities {
                println!("WRITTEN {:?}", written_variants(ability));
            }
        }
    }
}
