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
    "Arcee, Sharpshooter // Arcee, Acrobatic Coupe",
    "Avacyn's Judgment",
    "Battlefield Thaumaturge",
    "Benevolent River Spirit",
    "Blood Feud",
    "Bramble Elemental",
    "Breena, the Demagogue",
    "Case of the Burning Masks",
    "Cemetery Prowler",
    "Chandra, Fire of Kaladesh // Chandra, Roaring Flame",
    "Chaos Lord",
    "Charismatic Conqueror",
    "Creeping Bloodsucker",
    "Damocles Base, Sword of Kang",
    "Deepway Navigator",
    "Dig Up",
    "Dissension in the Ranks",
    "Draco",
    "Dread Fugue",
    "Dragon Grip",
    "Entropic Battlecruiser",
    "Essence Bottle",
    "Feast on the Fallen",
    "Fierce Retribution",
    "Fiery Bombardment",
    "First Response",
    "From Under the Floorboards",
    "Garland, Royal Kidnapper",
    "Gonti's Machinations",
    "Hatchery Sliver",
    "Heliod, the Radiant Dawn // Heliod, the Warped Eclipse",
    "Herald of Leshrac",
    "Herigast, Erupting Nullkite",
    "Heroes in a Half Shell",
    "Illuminate",
    "Imminent Doom",
    "Infinite Authority",
    "Inspired Idea",
    "Invasion of Fiora // Marchesa, Resolute Monarch",
    "Jetfire, Ingenious Scientist // Jetfire, Air Guardian",
    "Kaervek's Spite",
    "Katara, Seeking Revenge",
    "King Solomon's Frogs",
    "Korvold, Gleeful Glutton",
    "Krenko's Buzzcrusher",
    "Lantern Flare",
    "Light from Within",
    "Lumbering Megasloth",
    "Lunar Rejection",
    "Master of Cruelties",
    "Monastery Raid",
    "Multani, Maro-Sorcerer",
    "Naya Soulbeast",
    "Nicol Bolas, Dragon-God",
    "Ojer Axonil, Deepest Might // Temple of Power",
    "Paleontologist's Pick-Axe // Dinosaur Headdress",
    "Parasitic Grasp",
    "Path of Peril",
    "Pendant of Prosperity",
    "Planetarium of Wan Shi Tong",
    "Polukranos, Unchained",
    "Rampant Frogantua",
    "Reset",
    "Sokenzan Renegade",
    "Spiteful Visions",
    "Starnheim Unleashed",
    "Sutured Ghoul",
    "Syrix, Carrier of the Flame",
    "Tainted Specter",
    "Takeno, Samurai General",
    "The Legend of Yangchen // Avatar Yangchen",
    "The Temporal Anchor",
    "Thunderous Velocipede",
    "Umbra Stalker",
    "Vexing Puzzlebox",
    "Volo, Itinerant Scholar",
    "Voodoo Doll",
    "Vorinclex // The Grand Evolution",
    "Wash Away",
    "Water Whip",
    "Wayta, Trainer Prodigy",
    "Winged Portent",
    "Wolverine, Best There Is",
    "Xantcha, Sleeper Agent",
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
