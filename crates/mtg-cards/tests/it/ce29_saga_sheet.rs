//! Goal: on the read sheet every Saga chapter line ("I, II —", "III —") shows exactly the chapter abilities of the numbers it names, for every Saga of the corpus and every draft.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::ability::Ability;
use mtg_cards::card::Card;
use mtg_cards::load::parse_json;
use mtg_cards::support::PrintedCard;
use mtg_cards::trigger::TriggerPattern;

use crate::common::{Corpus, printed_of};

/// The chapter numbers a line opens with ("II, III — ..." is 2 and 3), or none for another line.
fn numerals(line: &str) -> Option<BTreeSet<u32>> {
    let (head, _) = line.split_once(" \u{2014} ")?;
    head.split(", ")
        .map(|numeral| {
            ["I", "II", "III", "IV", "V", "VI", "VII"]
                .iter()
                .position(|known| *known == numeral)
                .map(|at| u32::try_from(at).unwrap() + 1)
        })
        .collect()
}

/// The problems of the sheet of one card: each chapter line whose abilities are not the chapters it names.
fn problems(card: &Card, printed: &PrintedCard) -> Vec<String> {
    if !serde_json::to_string(card).unwrap().contains("Chapter") {
        return Vec::new();
    }
    let sheet = mtg_cards::read::sheet(card, printed);
    let mut found = Vec::new();
    for row in &sheet.rows {
        let Some(wanted) = numerals(&row.printed) else {
            continue;
        };
        if row.continues || row.abilities.is_empty() {
            continue;
        }
        let abilities = &card.faces[row.face].abilities;
        let shown: BTreeSet<u32> = row
            .abilities
            .iter()
            .flat_map(|&at| match abilities.get(at) {
                Some(Ability::Triggered {
                    when: TriggerPattern::Chapter { number },
                    ..
                }) => vec![*number],
                Some(Ability::Triggered {
                    when: TriggerPattern::AnyOf(any),
                    ..
                }) => any
                    .iter()
                    .filter_map(|pattern| match pattern {
                        TriggerPattern::Chapter { number } => Some(*number),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            })
            .collect();
        if shown != wanted {
            found.push(format!(
                "{}: {:?} shows chapters {shown:?}",
                printed.name, row.printed
            ));
        }
    }
    found
}

#[test]
fn each_chapter_line_shows_its_own_chapters() {
    let corpus = Corpus::repo();
    let library = corpus.library();
    let mut found = Vec::new();
    for record in corpus.records() {
        let printed = printed_of(&record);
        if let Some(card) = library.card(&printed.oracle_id) {
            found.extend(problems(card, &printed));
        }
    }
    let drafts = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../mtg-cli/drafts");
    for entry in std::fs::read_dir(&drafts).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        if !text.contains("Chapter") {
            continue;
        }
        let card: Card = parse_json(&path.to_string_lossy(), &text).unwrap();
        let fixture = drafts.join("fixtures").join(path.file_name().unwrap());
        let record_text = std::fs::read_to_string(&fixture).unwrap();
        let record = mtg_import::scryfall::cards(format!("[{record_text}]").as_bytes())
            .next()
            .unwrap()
            .unwrap();
        found.extend(problems(&card, &printed_of(&record)));
    }
    assert!(
        found.is_empty(),
        "{} lines:\n{}",
        found.len(),
        found.join("\n")
    );
}
