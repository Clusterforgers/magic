//! The gaps of the model-assisted reading that (r2bi) closed: the decks and games built from the corpus cards that take
//! control for as long as a source stays or is controlled, take groups, choose cards and move them, and count tokens by
//! what a player controls.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Basic lands in a deck of these cards, one color after the other.
const LANDS: usize = 17;

/// The corpus cards of these constructs, with their copies in a deck.
pub const C4_CARDS: &[(&str, usize)] = &[
    ("Mind Flayer", 1),
    ("Master Thief", 1),
    ("Sower of Temptation", 1),
    ("Opportunistic Dragon", 1),
    ("Zealous Conscripts", 1),
    ("Subjugate the Hobbits", 1),
    ("Unnatural Growth", 1),
    ("The Nipton Lottery", 1),
    ("Time Wipe", 1),
    ("Worldsoul's Rage", 1),
    ("Nervous Gardener", 1),
    ("Arachnogenesis", 1),
    ("Curious Herd", 1),
    ("Evangel of Heliod", 1),
];

/// A legal Normal deck of sixty cards: these cards, burn to answer them and creatures for the board, derived entirely
/// from `rng`.
pub fn c4_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in C4_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(burn("C4 Burn", 3), 3));
    // Ten names, so that no card passes the limit of four copies.
    let fodder: Vec<Arc<CardDefinition>> = (0..10)
        .map(|n| free_creature(&format!("C4 Creature {n}"), 1 + n % 3, 1 + n % 4))
        .collect();
    for card in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(card.clone());
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`c4_deck`]s, fully determined by `seed`.
pub fn c4_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(c4_deck(&mut GameRng::substream(seed, 360_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a deck of these cards is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_is_verified() {
        let deck = c4_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(deck.len(), 60);
        let corpus = corpus::corpus().unwrap();
        for &(name, copies) in C4_CARDS {
            assert_eq!(deck.iter().filter(|card| card.name == name).count(), copies);
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }

    #[test]
    fn a_seed_fixes_the_deck() {
        let first = c4_deck(&mut GameRng::substream(7, 0)).unwrap();
        let second = c4_deck(&mut GameRng::substream(7, 0)).unwrap();
        assert!(first.iter().zip(&second).all(|(a, b)| a.name == b.name));
    }
}
