//! Goal: scratch probe of how the compiler and the checker read named corpus cards (not committed).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use mtg_cards::Caps;
use mtg_cards::compile::compile;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::check;

#[test]
fn probe() {
    let Ok(list) = std::env::var("PROBE") else {
        return;
    };
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let names: std::collections::BTreeSet<String> = cards
        .iter()
        .flat_map(|c| std::iter::once(&c.name).chain(c.faces.iter().map(|f| &f.name)))
        .map(|n| n.to_lowercase())
        .collect();
    for name in list.split('|') {
        let Some(card) = cards.iter().find(|c| c.name == name) else {
            println!("== {name}: no fixture");
            continue;
        };
        println!("== {name}");
        let compiled = compile(card);
        for clause in &compiled.clauses {
            println!("  compile read={} {:?}", clause.read, clause.text);
        }
        if std::env::var("PROBE_JSON").is_ok()
            && let Some(rules) = &compiled.card
        {
            for face in &rules.faces {
                for ability in &face.abilities {
                    println!("  C {}", serde_json::to_string(ability).unwrap());
                }
            }
        }
        let path = common::cards_dir().join(format!("{}.json", slug(name)));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let report = check(&text, card, &tokens, &names, &Caps::everything());
        println!(
            "  verdict {:?} problems {:?}",
            report.verdict(),
            report.problems
        );
        for clause in &report.clauses {
            println!("  file {:?} :: {:?}", clause.text, clause.state);
        }
        for stray in &report.strays {
            println!("  stray {stray:?}");
        }
        if let Some(support) = &report.support {
            println!("  support {:?}", support.support);
        }
    }
}
