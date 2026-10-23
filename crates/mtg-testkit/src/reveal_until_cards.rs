//! Revealing until a card matches: the deck and games built from the corpus cards that do it.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 24;

/// The corpus cards that reveal cards from the top of the library until one matches, with their copies in a deck.
pub const REVEAL_UNTIL_CARDS: &[(&str, usize)] = &[
    ("Treasure Hunt", 4),
    ("Clifftop Lookout", 4),
    ("Evolutionary Leap", 4),
    ("Foster", 4),
];

/// A legal Normal deck of sixty cards of the list and plain creatures, derived entirely from `rng`.
pub fn reveal_until_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Green, Color::Blue];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in REVEAL_UNTIL_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    // Every name appears at most four times, which a legal deck allows.
    let mut slot = 0;
    while deck.len() < 60 {
        let power = 1 + slot % 4;
        let card = free_creature(&format!("Reveal Fodder {slot}"), power, power);
        deck.extend(std::iter::repeat_n(card, 2.min(60 - deck.len())));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`reveal_until_deck`]s, fully determined by `seed`.
pub fn reveal_until_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(reveal_until_deck(&mut GameRng::substream(
            seed,
            41_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a reveal-until deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = reveal_until_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = reveal_until_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in REVEAL_UNTIL_CARDS {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in REVEAL_UNTIL_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}: {:?}",
                corpus.support(name).unwrap()
            );
        }
    }
}
