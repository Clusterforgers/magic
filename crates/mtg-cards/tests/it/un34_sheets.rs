//! Goal: every Unfinity sticker sheet compiles whole into its stickers (three free name stickers, three art stickers and
//! a sticker for each ticket line), passes the checks of a rules file and reads back clause by clause as its text.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::ability::{Ability, Sticker};
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::compile;
use mtg_cards::generated::{Generation, generate};
use mtg_cards::load::Library;
use mtg_cards::proposal::differing;
use mtg_cards::support::PrintedCard;

/// The sheets of the fixture, as the importer reads them.
fn sheets() -> Vec<PrintedCard> {
    let path = common::fixture_dir().join("../un34_sheets.jsonl");
    let file = std::fs::File::open(&path).unwrap();
    mtg_import::scryfall::cards_jsonl(std::io::BufReader::new(file))
        .map(|record| common::printed_of(&record.unwrap()))
        .collect()
}

/// The sheets with a line no reader knows yet (`docs/cards-backlog.md`): they compile `Partial`.
const PARTIAL: [&str; 16] = [
    "Elemental Time Flamingo",
    "Eternal Acrobat Toast",
    "Misunderstood Trapeze Elf",
    "Mystic Doom Sandwich",
    "Notorious Sliver War",
    "Squid Fire Knight",
    "Trained Blessed Mind",
    "Unassuming Gelatinous Serpent",
    "Unhinged Beast Hunt",
    "Unique Charmed Pants",
    "Unstable Robot Dragon",
    "Vampire Champion Fury",
    "Weird Angel Flame",
    "Werewolf Lightning Mage",
    "Wild Ogre Bupkis",
    "Wrinkly Monkey Shenanigans",
];

/// What keeps a sheet's rules from passing the checks or reading back, or none; `whole` says whether every line read.
fn blocker(sheet: &PrintedCard, tokens: &Library) -> (bool, Option<String>) {
    let whole = compile(sheet).clauses.iter().all(|clause| clause.read);
    let names = BTreeSet::from([sheet.name.to_lowercase()]);
    let card = match generate(sheet, tokens, &names) {
        Generation::Rules(card) => card,
        other => return (whole, Some(format!("{other:?}"))),
    };
    let status = if whole {
        ImplStatus::Full
    } else {
        ImplStatus::Partial
    };
    if card.impl_status != status {
        return (whole, Some(format!("{:?}", card.impl_status)));
    }
    let differs = differing(&card, sheet);
    (
        whole,
        (!differs.is_empty()).then(|| format!("differs: {differs:?}")),
    )
}

#[test]
fn every_sheet_passes_the_checks_and_reads_back_and_all_but_the_partial_ones_whole() {
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let sheets = sheets();
    assert_eq!(sheets.len(), 48);
    let mut partial = Vec::new();
    let mut problems = Vec::new();
    for sheet in &sheets {
        let (whole, problem) = blocker(sheet, &tokens);
        if !whole {
            partial.push(sheet.name.as_str());
        }
        problems.extend(problem.map(|why| format!("{}: {why}", sheet.name)));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    partial.sort_unstable();
    assert_eq!(partial, PARTIAL);
}

#[test]
fn a_sheet_has_six_free_stickers_and_one_per_read_ticket_line() {
    for sheet in sheets() {
        let card = compile(&sheet).card.unwrap();
        let stickers: Vec<&Sticker> = card.faces[0]
            .abilities
            .iter()
            .filter_map(|ability| match ability {
                Ability::Sticker(sticker) => Some(sticker),
                _ => None,
            })
            .collect();
        let lines = sheet.faces[0].oracle_text.lines().count();
        let unread = card.faces[0]
            .abilities
            .iter()
            .filter(|ability| matches!(ability, Ability::Manual { .. }))
            .count();
        assert_eq!(stickers.len() + unread, 6 + lines, "{}", sheet.name);
        let paid: Vec<u32> = stickers
            .iter()
            .filter(|sticker| sticker.tickets() > 0)
            .map(|sticker| sticker.tickets())
            .collect();
        let printed: Vec<u32> = sheet.faces[0]
            .oracle_text
            .lines()
            .map(|line| {
                let cost = line.split(" \u{2014} ").next().unwrap();
                u32::try_from(cost.matches("{TK}").count()).unwrap()
            })
            .collect();
        if unread == 0 {
            assert_eq!(paid, printed, "{}", sheet.name);
        }
    }
}
