//! Goal: every card of the r28-0 pack reads back as its printed text: the checker accepts its rules file whole, so a
//! `Full` card is `Verified` by proof and a `Partial` card leaves only its `Manual` clauses.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeSet;

use mtg_cards::load::slug;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::Caps;

/// The cards of the pack.
const CARDS: &[&str] = &[
    "Angel's Herald",
    "Baton of Morale",
    "Beast Walkers",
    "Blind Seer",
    "Boom Scholar",
    "Chronatog",
    "Codie, Ravenous Codex",
    "Cursed Scroll",
    "Deadpool, Trading Card",
    "Deepwood Elder",
    "Demon's Herald",
    "Dragon's Herald",
    "Dragonkin Berserker",
    "Droning Bureaucrats",
    "Empyrial Archangel",
    "Experimental Frenzy",
    "Frodo, Sauron's Bane",
    "Genasi Enforcers",
    "Hellkite Overlord",
    "Icatian Infantry",
    "Immovable Rod",
    "Krasis Incubation",
    "Lita, Mechanical Engineer",
    "Magosi, the Waterveil",
    "Magus of the Scroll",
    "Nullhide Ferox",
    "Opal Acrolith",
    "Pack Mastiff",
    "Planeswalker's Favor",
    "Planeswalker's Fury",
    "Planeswalker's Mirth",
    "Planeswalker's Scorn",
    "Prince of Thralls",
    "Relic Amulet",
    "Shorecrasher Elemental",
    "Soraya the Falconer",
    "Soulstone Sanctuary",
    "Tenth District Hero",
    "Thelon of Havenwood",
    "Together Forever",
    "Torchling",
    "Wirefly Hive",
    "Xantcha, Sleeper Agent",
];

#[test]
fn every_r280_card_reads_back_as_its_text() {
    let library = common::Corpus::repo().library();
    let printed = common::printed();
    let names: BTreeSet<String> = printed
        .iter()
        .map(|card| card.name.to_lowercase())
        .collect();
    // `R280_ONLY` names the cards to check, joined by `|`, in place of the list.
    let only = std::env::var("R280_ONLY").unwrap_or_default();
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
        if report.verdict() != Verdict::Accepted || !plays || std::env::var("R280_SHOW").is_ok() {
            failed.push(report.render(true));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}
