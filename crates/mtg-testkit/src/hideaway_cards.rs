//! Hideaway: the deck and games built from the corpus cards that have the keyword.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 22;

/// The corpus cards with hideaway, with their copies in a deck.
pub const HIDEAWAY_CARDS: &[(&str, usize)] = &[
    ("Mosswort Bridge", 3),
    ("Fight Rigging", 2),
    ("Evercoat Ursine", 3),
    ("Smuggler's Buggy", 2),
    ("Cemetery Tampering", 1),
    ("Windbrisk Heights", 2),
    ("Spinerock Knoll", 2),
    ("Howltooth Hollow", 1),
    ("Clive's Hideaway", 1),
    ("Rabble Rousing", 1),
];

/// A legal Normal deck of sixty cards of the list, big creatures that meet their conditions, and burn, derived entirely from `rng`.
pub fn hideaway_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Green,
        Color::Black,
        Color::White,
        Color::Red,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in HIDEAWAY_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(burn("Burn", 2), 3));
    let fodder = [
        ("Titan", 7),
        ("Giant", 6),
        ("Brute", 5),
        ("Ogre", 5),
        ("Imp", 2),
        ("Wisp", 2),
        ("Troll", 4),
        ("Drake", 3),
    ];
    for (name, power) in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(free_creature(name, *power, *power));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`hideaway_deck`]s, fully determined by `seed`.
pub fn hideaway_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(hideaway_deck(&mut GameRng::substream(seed, 37_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a hideaway deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = hideaway_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = hideaway_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in HIDEAWAY_CARDS {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in HIDEAWAY_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}: {:?}",
                corpus.support(name).unwrap()
            );
        }
    }
}
