//! Characteristic and combat keywords (changeling, devoid, infect, wither, toxic, fear, intimidate, skulk, shadow and the
//! landwalks): the corpus cards that have them, and the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a combat-keyword deck, one color after the other.
const LANDS: usize = 25;

/// The corpus cards of a combat-keyword deck, with their copies.
pub const COMBAT_KEYWORD_CARDS: &[(&str, usize)] = &[
    ("Plague Stinger", 3),
    ("Pestilent Syphoner", 2),
    ("Bilious Skulldweller", 2),
    ("Blight Mamba", 2),
    ("Viridian Corrupter", 1),
    ("Midnight Banshee", 2),
    ("Boggart Ram-Gang", 2),
    ("Rustrazor Butcher", 2),
    ("Shadowmage Infiltrator", 2),
    ("Dauthi Horror", 2),
    ("Soltari Foot Soldier", 2),
    ("Furtive Homunculus", 2),
    ("Vampire Cutthroat", 2),
    ("Spectral Rider", 2),
    ("Krenko's Enforcer", 2),
    ("Cold-Eyed Selkie", 2),
    ("Zodiac Goat", 2),
    ("Bog Wraith", 2),
    ("Pale Bears", 2),
    ("Impostor of the Sixth Pride", 2),
    ("Slip Through Space", 2),
    ("Vestige of Emrakul", 2),
];

/// A legal Normal deck of the combat-keyword cards, creatures they meet in combat and burn, derived entirely from `rng`.
pub fn combat_keyword_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in COMBAT_KEYWORD_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(free_creature("Mite", 1, 1), 4));
    deck.extend(std::iter::repeat_n(bear(), 4));
    deck.extend(std::iter::repeat_n(burn("Burn", 3), 3));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`combat_keyword_deck`]s, fully determined by `seed`.
pub fn combat_keywords_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(combat_keyword_deck(&mut GameRng::substream(
            seed,
            24_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a combat-keyword deck holds every card of the list and enough cards to play, fixed by its seed.
    use super::*;

    #[test]
    fn a_deck_has_every_card_and_is_fixed_by_its_seed() {
        let first = combat_keyword_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = combat_keyword_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert!(first.len() >= 60, "{}", first.len());
        let names = |deck: &[Arc<CardDefinition>]| -> Vec<String> {
            deck.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in COMBAT_KEYWORD_CARDS {
            let held = first.iter().filter(|card| card.name == name).count();
            assert_eq!(held, copies, "{name}");
        }
    }
}
