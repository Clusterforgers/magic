//! Permanents that entered this turn: the decks and games built from the corpus cards that read them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land};
use crate::corpus::{self, CorpusError};

/// The corpus cards that read what entered this turn, with their copies in a deck.
pub const ENTERED_CARDS: &[(&str, usize)] = &[
    ("Octomancer", 3),
    ("Novijen, Heart of Progress", 2),
    ("Oran-Rief, the Vastwood", 3),
    ("Deathleaper, Terror Weapon", 3),
];

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 20;

/// A legal Normal deck of sixty cards of the list, small creatures that enter all game long, and burn, derived entirely
/// from `rng`.
pub fn entered_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Green, Color::Blue, Color::Red, Color::Black];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in ENTERED_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(burn("Burn", 2), 2));
    // Four copies of a name at most.
    let fodder = [
        ("Mite", 1),
        ("Gnat", 1),
        ("Brute", 2),
        ("Ogre", 3),
        ("Imp", 1),
        ("Wolf", 2),
        ("Hound", 2),
    ];
    for (name, power) in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(free_creature(name, *power, *power));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`entered_deck`]s, fully determined by `seed`.
pub fn entered_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(entered_deck(&mut GameRng::substream(seed, 29_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: an entered-this-turn deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = entered_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = entered_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in ENTERED_CARDS {
            let held = first.iter().filter(|card| card.name == name).count();
            assert_eq!(held, copies, "{name}");
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in ENTERED_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
