//! Goal: a probe that compiles the Scryfall objects of the directory `R116_FX` and prints each card's clauses that do not read (`R116_FULL` prints the whole rules of those that read, `R116_OUT` writes them as rules files there); silent without it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_import::scryfall::cards;

use crate::common::printed_of;

#[test]
fn r116_probe_lists_unread_clauses() {
    let Ok(dir) = std::env::var("R116_FX") else {
        return;
    };
    let full = std::env::var("R116_FULL").is_ok();
    let mut read = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        for record in cards(format!("[{text}]").as_bytes()).flatten() {
            let card = printed_of(&record);
            let compiled = mtg_cards::compile::compile(&card);
            let unread: Vec<_> = compiled
                .clauses
                .iter()
                .filter(|clause| !clause.read && !clause.held_by_others)
                .collect();
            if unread.is_empty() {
                read += 1;
                println!("READ {}", card.name);
                if let Some(rules) = &compiled.card {
                    let text = mtg_cards::pretty::pretty(&mtg_cards::load::canonical(rules));
                    if full {
                        println!("{text}");
                    }
                    if let Ok(out) = std::env::var("R116_OUT") {
                        let slug = mtg_cards::load::slug(&card.name);
                        std::fs::write(format!("{out}/{slug}.json"), format!("{text}\n")).unwrap();
                    }
                }
            }
            for clause in unread {
                println!("UNREAD {}\t{:?}\t{}", card.name, clause.kind, clause.text);
            }
        }
    }
    println!("READ TOTAL {read}");
}
