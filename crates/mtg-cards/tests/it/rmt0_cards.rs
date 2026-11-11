//! Goal: every card of the rmt0 pack reads back as its printed text: the checker accepts its rules file whole, so a
//! `Full` card is `Verified` by proof and a `Partial` card leaves only its `Manual` clauses.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use mtg_cards::load::slug;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::Caps;

/// The cards of the pack.
const CARDS: &[&str] = &[
    "Alchemist's Retrieval",
    "Angel of Destiny",
    "Benevolent River Spirit",
    "Blood Feud",
    "Bramble Elemental",
    "Breena, the Demagogue",
    "Charismatic Conqueror",
    "Creeping Bloodsucker",
    "Damocles Base, Sword of Kang",
    "Dig Up",
    "Dissension in the Ranks",
    "Draco",
    "Dragon Grip",
    "Essence Bottle",
    "Feast on the Fallen",
    "Fierce Retribution",
    "Fiery Bombardment",
    "First Response",
    "Gonti's Machinations",
    "Heroes in a Half Shell",
    "Imminent Doom",
    "Infinite Authority",
    "Inspired Idea",
    "Katara, Seeking Revenge",
    "King Solomon's Frogs",
    "Krenko's Buzzcrusher",
    "Light from Within",
    "Multani, Maro-Sorcerer",
    "Naya Soulbeast",
    "Parasitic Grasp",
    "Path of Peril",
    "Pendant of Prosperity",
    "Planetarium of Wan Shi Tong",
    "Reset",
    "Sokenzan Renegade",
    "Spiteful Visions",
    "Syrix, Carrier of the Flame",
    "Tainted Specter",
    "The Legend of Yangchen // Avatar Yangchen",
    "Umbra Stalker",
    "Vexing Puzzlebox",
    "Voodoo Doll",
    "Water Whip",
    "Winged Portent",
    "Yotia Declares War",
];

#[test]
fn every_rmt0_card_reads_back_as_its_text() {
    let library = common::Corpus::repo().library();
    let printed = common::printed();
    let names: BTreeSet<String> = printed
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    // `RMT0_ONLY` names the cards to check, joined by `|`, in place of the list.
    let only = std::env::var("RMT0_ONLY").unwrap_or_default();
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
        if report.verdict() != Verdict::Accepted || !plays || std::env::var("RMT0_SHOW").is_ok() {
            failed.push(report.render(true));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}

/// Writes the compiler's reading of each named card that has no rules file yet (`RMT0_DRAFT=<dir>`), as a start.
#[test]
fn drafts_of_missing_rmt0_cards() {
    let Ok(dir) = std::env::var("RMT0_DRAFT") else {
        return;
    };
    let wanted = std::env::var("RMT0_ONLY").unwrap_or_default();
    for card in common::printed() {
        let file = format!("{}.json", slug(&card.name));
        if common::cards_dir().join(&file).exists() || !wanted.split('|').any(|w| w == card.name) {
            continue;
        }
        if let Some(rules) = mtg_cards::compile::compile(&card).card {
            let out = std::path::Path::new(&dir).join(&file);
            std::fs::write(out, mtg_cards::load::canonical(&rules)).unwrap();
        }
        // A cleave card: its text with the brackets dropped, and without the words in them.
        let text = card.faces[0].oracle_text.clone();
        if text.contains('[') {
            for (tag, cut) in [("full", false), ("short", true)] {
                let mut kept = String::new();
                let mut inside = false;
                for c in text.chars() {
                    match c {
                        '[' => inside = true,
                        ']' => inside = false,
                        c if !(inside && cut) => kept.push(c),
                        _ => {}
                    }
                }
                let mut changed = card.clone();
                changed.faces[0].oracle_text = kept.replace("  ", " ").replace(" .", ".");
                if let Some(rules) = mtg_cards::compile::compile(&changed).card {
                    let out = std::path::Path::new(&dir).join(format!("{tag}_{file}"));
                    std::fs::write(out, mtg_cards::load::canonical(&rules)).unwrap();
                }
            }
        }
    }
}
