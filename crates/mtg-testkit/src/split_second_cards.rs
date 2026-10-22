//! A seeded deck of the corpus cards with split second and undaunted, beside permanents with activated, mana and triggered
//! abilities, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, creature, land};
use crate::corpus;

/// 60 cards of two copies of each card of the list, 22 lands and the rest of the bears, drawn from `rng`.
fn split_second_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [
        Color::Red,
        Color::Black,
        Color::Blue,
        Color::Green,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..22).map(|i| land(colors[i % 5])).collect();
    let mut cards: Vec<Arc<CardDefinition>> = [
        "Sudden Shock",
        "Krosan Grip",
        "Sudden Death",
        "Sudden Edict",
        "Wipe Away",
        "Stonewood Invocation",
        "Celestial Crusader",
        "Sulfur Elemental",
        "Siege Smash",
        "Sublime Exhalation",
        "Curtains' Call",
        "Seeds of Renewal",
        "Coastal Breach",
        "Lightning Bolt",
        "Counterspell",
        "Mother of Runes",
        "Monastery Swiftspear",
        "Llanowar Elves",
        "Sol Ring",
    ]
    .iter()
    .filter_map(|name| corpus::card(name).ok())
    .collect();
    cards.push(burn("Bolt", 3));
    cards.push(creature("Plain Bear", 2, &[Color::Green], 2, 2));
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`split_second_deck`]s, fully determined by `seed`.
pub fn split_second_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| split_second_deck(&mut GameRng::substream(seed, 35_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
