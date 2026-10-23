//! A seeded Commander deck of the corpus cards that are cast without paying their mana costs while their caster controls a
//! commander and of the corpus card with strive, with a five-color commander, lands of every color, cheap creatures to aim them
//! at and artifacts to destroy, for fuzz games.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Commander;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::commander_cards::legend;
use crate::corpus::{self, CorpusError};

/// The corpus cards cast without paying their mana costs while their caster controls a commander.
pub const FREE_CAST_CARDS: [&str; 4] = [
    "Fierce Guardianship",
    "Deadly Rollick",
    "Flawless Maneuver",
    "Obscuring Haze",
];

/// The corpus cards with strive.
pub const STRIVE_CARDS: [&str; 1] = ["Consign to Dust"];

/// Artifacts and enchantments for strive to destroy.
const TARGETS: [&str; 3] = ["Sol Ring", "Arcane Signet", "Swiftfoot Boots"];

/// A legal Commander deck of the 99 cards besides its commander: the cards above, a few targets for strive, distinct cheap
/// creatures and basic lands of the five colors, derived entirely from `rng`. The commander is a cheap five-color legend.
pub fn free_cast_deck(
    rng: &mut GameRng,
    seat: u64,
) -> Result<(Vec<Arc<CardDefinition>>, Arc<CardDefinition>), CorpusError> {
    let commander = legend(&format!("Prism Chief {seat}"), 1, &[Color::Red], 2, 2);
    let commander = Arc::new(
        CardDefinition::clone(&commander).with_color_indicator(
            mtg_core::color::ColorSet::EMPTY
                .with(Color::White)
                .with(Color::Blue)
                .with(Color::Black)
                .with(Color::Red)
                .with(Color::Green),
        ),
    );
    let mut deck = Vec::new();
    for name in FREE_CAST_CARDS.iter().chain(&STRIVE_CARDS).chain(&TARGETS) {
        deck.push(corpus::card(name)?);
    }
    for slot in 0..32 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let color = Color::ALL[slot % Color::ALL.len()];
        deck.push(creature(
            &format!("Free Cast Fodder {seat} {slot}"),
            1,
            &[color],
            power,
            power,
        ));
    }
    let mut next = 0;
    while deck.len() < 99 {
        deck.push(land(Color::ALL[next % Color::ALL.len()]));
        next += 1;
    }
    rng.shuffle(&mut deck);
    Ok((deck, commander))
}

/// A Commander game of `players` seats whose decks are [`free_cast_deck`]s, fully determined by `seed`.
pub fn free_cast_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    let mut commanders = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        let (deck, commander) = free_cast_deck(&mut GameRng::substream(seed, 36_000 + seat), seat)?;
        decks.push(deck);
        commanders.push(vec![commander]);
    }
    Ok(GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders))
}
