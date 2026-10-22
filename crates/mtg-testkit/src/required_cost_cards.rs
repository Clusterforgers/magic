//! A seeded deck of the corpus cards with a cost the caster must pay in addition ("As an additional cost to cast this
//! spell, discard a card", sacrifice a creature, a land or an artifact, pay 3 life, exile creature cards from a
//! graveyard), with lands of their colours, artifacts and creatures to give and to aim them at, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus;

/// What a required additional cost takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Takes {
    /// Cards discarded from the hand.
    Discard(usize),
    /// Permanents sacrificed.
    Sacrifice(usize),
    /// Cards exiled from the graveyard.
    Exile(usize),
    /// Life paid.
    Life(u32),
}

/// The corpus cards with a required additional cost, and what each takes.
pub const REQUIRED_COST_CARDS: &[(&str, Takes)] = &[
    ("Thrill of Possibility", Takes::Discard(1)),
    ("Electric Revelation", Takes::Discard(1)),
    ("Laughing Mad", Takes::Discard(1)),
    ("Cathartic Reunion", Takes::Discard(2)),
    ("Altar's Reap", Takes::Sacrifice(1)),
    ("Village Rites", Takes::Sacrifice(1)),
    ("Crop Rotation", Takes::Sacrifice(1)),
    ("Harrow", Takes::Sacrifice(1)),
    ("Reshape", Takes::Sacrifice(1)),
    ("Trash for Treasure", Takes::Sacrifice(1)),
    ("Withering Boon", Takes::Life(3)),
    ("Weigh Down", Takes::Exile(1)),
    ("Makeshift Mauler", Takes::Exile(1)),
    ("Stitched Drake", Takes::Exile(1)),
    ("Headless Skaab", Takes::Exile(1)),
    ("Relentless Skaabs", Takes::Exile(1)),
    ("Skaab Goliath", Takes::Exile(2)),
];

/// What the card called `name` takes as an additional cost, when it has one among [`REQUIRED_COST_CARDS`].
pub fn required_cost_of(name: &str) -> Option<Takes> {
    REQUIRED_COST_CARDS
        .iter()
        .find(|(card, _)| *card == name)
        .map(|(_, takes)| *takes)
}

/// A legal Normal deck of lands, two of each required-cost card, artifacts to sacrifice and creatures to give and to aim
/// at, derived entirely from `rng`.
pub fn required_cost_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let colors = [Color::Blue, Color::Red, Color::Black, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..20).map(|i| land(colors[i % 4])).collect();
    for (name, _) in REQUIRED_COST_CARDS {
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, 2));
        }
    }
    for name in ["Sol Ring", "Bonesplitter"] {
        if let Ok(card) = corpus::card(name) {
            deck.extend(std::iter::repeat_n(card, 2));
        }
    }
    for slot in 0..6 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Required Fodder {slot} {power}/{power}");
        let card = creature(&name, 1, &[colors[rng.index(4)]], power, power);
        deck.extend(std::iter::repeat_n(card, 2));
    }
    rng.shuffle(&mut deck);
    deck.truncate(60);
    deck
}

/// A normal game of `players` seats whose decks are [`required_cost_deck`]s, fully determined by `seed`.
pub fn required_cost_fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| required_cost_deck(&mut GameRng::substream(seed, 28_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}
