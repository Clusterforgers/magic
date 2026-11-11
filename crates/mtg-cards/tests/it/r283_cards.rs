//! Goal: every card of the r28-3 pack reads back as its printed text: the checker accepts its rules file whole, so a
//! `Full` card is `Verified` by proof and a `Partial` card leaves only its `Manual` clauses.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use mtg_cards::compile::compile;
use mtg_cards::load::{canonical, slug};
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::Caps;
use mtg_import::scryfall::cards;

/// The cards of the pack.
const CARDS: &[&str] = &[
    "Amass the Components",
    "Angrath's Fury",
    "Avacyn's Judgment",
    "Beacon of Immortality",
    "Calamity of the Titans",
    "Catch // Release",
    "Contact Other Plane",
    "Crush of Tentacles",
    "Cut the Earthly Bond",
    "Dovin's Dismissal",
    "Enter the Infinite",
    "Errand of Duty",
    "Farideh's Fireball",
    "Feast of Dreams",
    "Fell Beast's Shriek",
    "Formation",
    "From Under the Floorboards",
    "Gloomwidow's Feast",
    "Grip of Amnesia",
    "Hezrou // Demonic Stench",
    "Hustle // Bustle",
    "Illuminate",
    "Laquatus's Disdain",
    "Lava Blister",
    "March of Souls",
    "Misinformation",
    "Monastery Raid",
    "Outwit",
    "Overwhelming Encounter",
    "Perplex",
    "Pistus Strike",
    "Pull from Eternity",
    "Raphael's Technique",
    "Reciprocate",
    "Recruitment Drive",
    "Rise // Fall",
    "Rise from the Wreck",
    "Runic Repetition",
    "Scout for Survivors",
    "Spider-Sense",
    "Starnheim Unleashed",
    "The Crowd Goes Wild",
    "The Last Ronin's Technique",
    "Titan's Presence",
    "Venomous Vines",
    "Winds of Change",
];

#[test]
fn every_r283_card_reads_back_as_its_text() {
    let library = common::Corpus::repo().library();
    let printed = common::printed();
    let names: BTreeSet<String> = printed
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    // `R283_ONLY` names the cards to check, joined by `|`, in place of the list.
    let only = std::env::var("R283_ONLY").unwrap_or_default();
    let wanted: Vec<&str> = match only.as_str() {
        "" => CARDS.to_vec(),
        only => only.split('|').collect(),
    };
    let mut failed = Vec::new();
    for name in wanted {
        let card = printed
            .iter()
            .find(|card| card.name == name)
            .unwrap_or_else(|| panic!("no fixture for {name}"));
        let path = common::cards_dir().join(format!("{}.json", slug(name)));
        let text = std::fs::read_to_string(&path).unwrap();
        let report = check_exact(&text, card, &library, &names, &Caps::everything());
        let plays = ["ACCEPTED Verified", "ACCEPTED Partial"].contains(&report.summary().as_str());
        if report.verdict() != Verdict::Accepted || !plays || std::env::var("R283_SHOW").is_ok() {
            failed.push(report.render(true));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}

/// A tool, not a check: `R283_PROBE=<dir of Scryfall objects>` compiles each card and checks the rules file of the
/// same slug in `R283_RULES=<dir>` (else the compiled rules), printing both.
#[test]
#[ignore = "a probe for writing the pack's cards"]
fn probe() {
    let Ok(dir) = std::env::var("R283_PROBE") else {
        return;
    };
    let rules = std::env::var("R283_RULES").unwrap_or_default();
    let filter = std::env::var("R283_ONLY").unwrap_or_default();
    let library = common::Corpus::repo().library();
    let mut names: BTreeSet<String> = common::printed()
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    let mut out = String::new();
    for path in entries {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if !filter.is_empty() && !filter.split('|').any(|f| f == stem) {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let record = cards(format!("[{text}]").as_bytes())
            .next()
            .unwrap()
            .unwrap();
        let printed = common::printed_of(&record);
        names.insert(printed.name.to_lowercase());
        let compiled = compile(&printed);
        let compiled_text = compiled.card.as_ref().map(canonical).unwrap_or_default();
        let file = std::path::Path::new(&rules).join(format!("{stem}.json"));
        let (source, proposal) = match std::fs::read_to_string(&file) {
            Ok(own) => ("file", own),
            Err(_) => ("compiled", compiled_text.clone()),
        };
        let report = check_exact(&proposal, &printed, &library, &names, &Caps::everything());
        let _ = writeln!(out, "=== {} [{source}] {}", printed.name, report.summary());
        if std::env::var("R283_JSON").is_ok() || source == "compiled" {
            out.push_str(&compiled_text);
            out.push('\n');
        }
        out.push_str(&report.render(true));
        out.push('\n');
    }
    let target = std::env::var("R283_OUT").unwrap_or_else(|_| "/dev/stdout".into());
    std::fs::write(target, out).unwrap();
}
