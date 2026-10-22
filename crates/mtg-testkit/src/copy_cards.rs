//! Decks for the games that exercise copies: the cards that enter as a copy, make token copies and populate, with
//! creatures to copy, token makers to populate from and spells to aim at the copies.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};
use crate::corpus::{self, CorpusError};

/// A deck of lands in three colours, Clone, Phantasmal Image, Mirror Image, Cackling Counterpart, Rootborn Defenses,
/// Growing Ranks and Followed Footsteps from the corpus, Hero of Bladehold and Questing Beast to copy and populate
/// from, Lightning Bolts to aim at them, and creatures, derived entirely from `rng`.
pub fn copy_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Blue, Color::White, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> = (0..22).map(|i| land(colors[i % 3])).collect();
    for (name, copies) in [
        ("Clone", 3),
        ("Phantasmal Image", 3),
        ("Mirror Image", 2),
        ("Cackling Counterpart", 3),
        ("Rootborn Defenses", 3),
        ("Growing Ranks", 2),
        ("Followed Footsteps", 2),
        ("Hero of Bladehold", 3),
        ("Lightning Bolt", 4),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let generic = u32::try_from(rng.index(3)).unwrap_or(0);
        let name = format!("Copy Fodder {slot} {generic} {power}/{power}");
        let card = creature(&name, generic, &[colors[rng.index(3)]], power, power);
        let copies = (60 - deck.len()).min(2);
        deck.extend(std::iter::repeat_n(card, copies));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`copy_deck`]s, fully determined by `seed`.
pub fn copy_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(copy_deck(&mut GameRng::substream(seed, 12_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}
