//! A seeded deck of the cards that give keywords to the cards of a hand (Rhet-Tomb Mystic, Tectonic Reformation, Jo
//! Grant, Molecule Man), the creature, land and historic cards they give them to, and fillers, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, creature, elf, giant, land};
use crate::corpus;

/// A legal Normal deck of the four granting cards twice each, cards to give the keywords to (creature, land, noncreature
/// and legendary ones) and burn, derived entirely from `rng`.
pub fn card_grants_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [
        Color::Blue,
        Color::Red,
        Color::White,
        Color::Black,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..26).map(|i| land(colors[i % 5])).collect();
    let mut cards: Vec<Arc<CardDefinition>> = [
        "Rhet-Tomb Mystic",
        "Tectonic Reformation",
        "Jo Grant",
        "Molecule Man",
        "Llanowar Elves",
        "Sol Ring",
    ]
    .iter()
    .filter_map(|name| corpus::card(name).ok())
    .collect();
    cards.extend([
        bear(),
        elf(),
        giant(),
        burn("Bolt", 3),
        creature("Plain Bear", 2, &[Color::Green], 2, 2),
        creature("Blue Drake", 3, &[Color::Blue], 2, 2),
        creature("Red Lizard", 1, &[Color::Red], 2, 1),
        creature("White Knight", 2, &[Color::White], 2, 2),
        creature("Black Rat", 1, &[Color::Black], 1, 1),
        creature("Green Wolf", 2, &[Color::Green], 3, 2),
        creature("Blue Frog", 2, &[Color::Blue], 1, 3),
    ]);
    for card in cards {
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`card_grants_deck`]s, fully determined by `seed`.
pub fn card_grants_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| card_grants_deck(&mut GameRng::substream(seed, 310_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
