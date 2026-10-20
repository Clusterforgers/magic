//! Goal: over a full Oracle Cards download, the compiler never panics and every card it writes loads and matches its imported text.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::io::BufReader;

use mtg_cards::compile::{ClauseKind, compile};
use mtg_cards::load::{Library, Source, canonical, slug};

/// A clause with its numbers as N, its mana symbols as {M} and its lines as " / " (the name is `~` already).
fn shape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let symbol: String = chars.by_ref().take_while(|next| *next != '}').collect();
            out.push_str(if matches!(symbol.as_str(), "T" | "Q") {
                "{T}"
            } else {
                "{M}"
            });
        } else if c.is_ascii_digit() {
            while chars.next_if(char::is_ascii_digit).is_some() {}
            out.push('N');
        } else if c == '\n' {
            out.push_str(" / ");
        } else {
            out.push(c);
        }
    }
    out
}

/// Prints, without asserting, the share of clauses read for each kind and the 15 most common
/// unread spell-clause shapes.
fn print_shares(kinds: &BTreeMap<String, (usize, usize)>, unread_spells: BTreeMap<String, usize>) {
    eprintln!("share of clauses read, by kind:");
    for (kind, (total, read)) in kinds {
        let share = 100.0 * f64::from(u32::try_from(*read).unwrap())
            / f64::from(u32::try_from(*total).unwrap().max(1));
        eprintln!("  {kind:<10} {read:>6} of {total:>6} ({share:.1}%)");
    }
    let mut shapes: Vec<_> = unread_spells.into_iter().collect();
    shapes.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    eprintln!("the 15 most common unread spell-clause shapes:");
    for (shape, count) in shapes.iter().take(15) {
        eprintln!("  {count:>5}  {shape}");
    }
}

/// Ignored by default (needs a download). Set `MTG_REAL_DATA_DIR` to a directory holding
/// `oracle-cards-*.jsonl.gz` and run `cargo test -p mtg-cards --test compile_real -- --ignored --nocapture`.
#[test]
#[ignore = "needs MTG_REAL_DATA_DIR"]
fn every_card_of_a_full_download_compiles_to_rules_that_load() {
    let dir = std::path::PathBuf::from(
        std::env::var("MTG_REAL_DATA_DIR").expect("set MTG_REAL_DATA_DIR"),
    );
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with("oracle-cards-") && name.ends_with(".jsonl.gz")
        })
        .collect();
    files.sort();
    let file = std::fs::File::open(files.pop().expect("an oracle-cards-*.jsonl.gz file")).unwrap();
    let cards: Vec<_> =
        mtg_import::scryfall::cards_jsonl(flate2::read::GzDecoder::new(BufReader::new(file)))
            .filter_map(Result::ok)
            .map(|record| common::printed_of(&record))
            .collect();
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut problems: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unread_keywords: BTreeMap<String, usize> = BTreeMap::new();
    let mut unread_spells: BTreeMap<String, usize> = BTreeMap::new();
    for printed in &cards {
        let compiled = compile(printed);
        for clause in &compiled.clauses {
            let entry = kinds.entry(format!("{:?}", clause.kind)).or_default();
            entry.0 += 1;
            entry.1 += usize::from(clause.read);
            if clause.kind == ClauseKind::Keyword && !clause.read {
                *unread_keywords.entry(clause.text.clone()).or_default() += 1;
            }
            if clause.kind == ClauseKind::Spell && !clause.read {
                *unread_spells.entry(shape(&clause.text)).or_default() += 1;
            }
            let text = &printed.faces[clause.face].oracle_text;
            assert!(clause.span.slice(text).is_some(), "{}", printed.name);
        }
        let Some(card) = compiled.card else {
            *statuses.entry("no rules text".into()).or_default() += 1;
            continue;
        };
        *statuses
            .entry(format!("{:?}", card.impl_status))
            .or_default() += 1;
        let source = Source {
            path: format!("cards/{}.json", slug(&card.name)),
            text: canonical(&card),
        };
        match Library::from_sources(&[source]) {
            Ok(library) => {
                for error in library.check_printed(std::slice::from_ref(printed)) {
                    // A partner's name is a card of its own, which a single card cannot see.
                    let message = if error.message.starts_with("no imported card is named") {
                        "partner not in the batch".to_owned()
                    } else {
                        error.message.chars().take(60).collect()
                    };
                    problems
                        .entry(message)
                        .or_default()
                        .push(printed.name.clone());
                }
            }
            Err(errors) => {
                problems
                    .entry(format!(
                        "load: {}",
                        errors.0[0].message.chars().take(60).collect::<String>()
                    ))
                    .or_default()
                    .push(printed.name.clone());
            }
        }
    }
    eprintln!(
        "{} cards\nstatuses {statuses:#?}\nkinds (clauses, read) {kinds:#?}",
        cards.len()
    );
    for (message, names) in &problems {
        eprintln!(
            "{} x {message}: {:?}",
            names.len(),
            &names[..names.len().min(5)]
        );
    }
    let mut top: Vec<_> = unread_keywords.into_iter().collect();
    top.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    eprintln!("unread keyword lines {:#?}", &top[..top.len().min(40)]);
    print_shares(&kinds, unread_spells);
    let unexpected: Vec<_> = problems
        .keys()
        .filter(|message| *message != "partner not in the batch")
        .collect();
    assert!(unexpected.is_empty(), "{unexpected:?}");
}
