//! A seeded deck of the corpus cards that pay costs beyond mana and a kicker's mana: Capsize (buyback), Dream's Grip
//! (entwine), Voltage Surge (an optional additional cost that sacrifices), Vicious Offering (a kicker that sacrifices),
//! Deep Analysis (a flashback that pays life) and Endurance (an evoke that exiles a card from the hand), with lands of
//! their colours, artifacts to sacrifice and creatures to aim them at, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// The corpus cards that pay costs with parts other than mana, and the artifact they sacrifice.
pub const EXTRA_COST_CARDS: [&str; 7] = [
    "Capsize",
    "Dream's Grip",
    "Voltage Surge",
    "Vicious Offering",
    "Deep Analysis",
    "Endurance",
    "Bonesplitter",
];

/// A legal Normal deck of lands, four of each card and creatures to aim them at, derived entirely from `rng`.
pub fn extra_cost_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Red, Color::Black, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..24).map(|i| land(colors[i % 4])).collect();
    for card in EXTRA_COST_CARDS
        .iter()
        .filter_map(|name| corpus::card(name).ok())
    {
        deck.extend(std::iter::repeat_n(card, 3));
    }
    for slot in 0..8 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Extra Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(4)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`extra_cost_deck`]s, fully determined by `seed`.
pub fn extra_cost_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| extra_cost_deck(&mut GameRng::substream(seed, 21_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
