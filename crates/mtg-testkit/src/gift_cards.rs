//! Gift: the decks and games built from the corpus cards that have the keyword.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of gift cards, one color after the other.
const LANDS: usize = 22;

/// The corpus cards with gift, with their copies in a deck.
pub const GIFT_CARDS: &[(&str, usize)] = &[
    ("Scrapshooter", 2),
    ("Wear Down", 2),
    ("Peerless Recycling", 2),
    ("Long River's Pull", 2),
    ("Sazacap's Brew", 2),
    ("Consumed by Greed", 2),
];

/// A legal Normal deck of sixty cards: the gift cards, artifacts for them to destroy, creatures and burn, derived
/// entirely from `rng`.
pub fn gift_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Red,
        Color::Black,
        Color::Green,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in GIFT_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(corpus::card("Bonesplitter")?, 4));
    deck.extend(std::iter::repeat_n(burn("Burn", 2), 2));
    let fodder = [
        ("Mite", 1),
        ("Gnat", 1),
        ("Brute", 3),
        ("Ogre", 3),
        ("Imp", 2),
    ];
    for (name, power) in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(free_creature(name, *power, *power));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`gift_deck`]s, fully determined by `seed`.
pub fn gift_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(gift_deck(&mut GameRng::substream(seed, 29_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a gift deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    #[test]
    fn a_deck_has_sixty_cards_and_every_gift_card_and_the_corpus_cards_are_verified() {
        let deck = gift_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(deck.len(), 60);
        let corpus = corpus::corpus().unwrap();
        for &(name, copies) in GIFT_CARDS {
            assert_eq!(deck.iter().filter(|card| card.name == name).count(), copies);
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
