//! Goal: a probe that compiles the Scryfall objects of the directory `R305_FX` and checks the rules files named in `R305_RULES` (comma separated) against them or the corpus; silent without either.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::load::Library;
use mtg_cards::proposal::check;
use mtg_cards::support::{Caps, PrintedCard};
use mtg_import::scryfall::cards;

use crate::common::{printed, printed_of, token_sources};

fn extra() -> Vec<PrintedCard> {
    let Ok(dir) = std::env::var("R305_FX") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        for record in cards(format!("[{text}]").as_bytes()).flatten() {
            out.push(printed_of(&record));
        }
    }
    out
}

#[test]
fn probe_compiles_and_checks() {
    let fx = extra();
    if let Ok(filter) = std::env::var("R305_COMPILE") {
        for card in fx
            .iter()
            .filter(|c| filter.is_empty() || filter.contains(&c.name))
        {
            let compiled = mtg_cards::compile::compile(card);
            let text = compiled
                .card
                .map(|rules| mtg_cards::pretty::pretty(&mtg_cards::load::canonical(&rules)))
                .unwrap_or_default();
            println!("== {}\n{text}", card.name);
        }
    }
    let Ok(list) = std::env::var("R305_RULES") else {
        return;
    };
    let mut all = printed();
    all.extend(fx);
    let names: BTreeSet<String> = all
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    for path in list.split(',').filter(|p| !p.is_empty()) {
        let text = std::fs::read_to_string(path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let name = value["name"].as_str().unwrap_or_default().to_string();
        let Some(card) = all.iter().find(|card| card.name == name) else {
            println!("== {path}: no fixture for {name}");
            continue;
        };
        let report = check(&text, card, &tokens, &names, &Caps::everything());
        println!("== {name}: {:?}\n{}", report.verdict(), report.render(true));
    }
}
