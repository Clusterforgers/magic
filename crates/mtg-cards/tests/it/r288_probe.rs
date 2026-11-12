//! Goal: a probe that prints the checker's report for the rules files named in `R288_PROBE` (paths, comma separated); silent without it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::load::Library;
use mtg_cards::proposal::check;
use mtg_cards::support::Caps;

use crate::common::{printed, token_sources};

#[test]
fn probe_prints_the_report_of_named_files() {
    let Ok(list) = std::env::var("R288_PROBE") else {
        return;
    };
    let cards = printed();
    let names: BTreeSet<String> = cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    for path in list.split(',').filter(|p| !p.is_empty()) {
        let text = std::fs::read_to_string(path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let name = value["name"].as_str().unwrap_or_default().to_string();
        let Some(card) = cards.iter().find(|card| card.name == name) else {
            println!("== {path}: no fixture for {name}");
            continue;
        };
        let report = check(&text, card, &tokens, &names, &Caps::everything());
        println!("== {name}: {:?}\n{}", report.verdict(), report.render(true));
    }
}

#[test]
fn probe_prints_the_compiled_rules_of_named_cards() {
    let Ok(list) = std::env::var("R288_COMPILE") else {
        return;
    };
    let cards = printed();
    for name in list.split('|').filter(|n| !n.is_empty()) {
        let Some(card) = cards.iter().find(|card| card.name == name) else {
            println!("== {name}: no fixture");
            continue;
        };
        let compiled = mtg_cards::compile::compile(card);
        let text = compiled
            .card
            .map(|rules| mtg_cards::pretty::pretty(&mtg_cards::load::canonical(&rules)))
            .unwrap_or_default();
        println!("== {name}\n{text}");
    }
}

#[test]
fn probe_prints_the_wordings_of_abilities() {
    let Ok(path) = std::env::var("R288_ABILITY") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<mtg_cards::ability::Ability>(line) {
            Ok(ability) => {
                println!("== {line}");
                for said in mtg_cards::compile::written_variants(&ability) {
                    println!("   {said}");
                }
            }
            Err(e) => println!("== {line}\n   parse error: {e}"),
        }
    }
}
