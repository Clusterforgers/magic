//! The blockers that alone held back two cards each: the decks and games built from the corpus cards that half a life total,
//! a return at the next end step, the most cards discarded and a card exiled from a graveyard freed.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land, salve};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 22;

/// The corpus cards of these blockers, with their copies in a deck.
pub const PAIR_CARDS: &[(&str, usize)] = &[
    ("Quietus Spike", 1),
    ("Scytheclaw", 1),
    ("The Locust God", 1),
    ("The Scorpion God", 1),
    ("Windfall", 2),
    ("Whispering Madness", 1),
    ("Jace's Archivist", 1),
    ("Relic of Progenitus", 1),
    ("Scrabbling Claws", 1),
];

/// A legal Normal deck of sixty cards: these cards, life gain and burn for them to answer, and creatures, derived entirely
/// from `rng`.
pub fn pair_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in PAIR_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(salve("Pair Salve"), 3));
    deck.extend(std::iter::repeat_n(burn("Pair Burn", 4), 3));
    let fodder = [
        ("Mite", 1),
        ("Gnat", 1),
        ("Brute", 3),
        ("Ogre", 3),
        ("Imp", 2),
        ("Wisp", 1),
        ("Newt", 1),
        ("Boar", 3),
        ("Crab", 2),
        ("Wolf", 2),
    ];
    for (name, power) in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(free_creature(name, *power, *power));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`pair_deck`]s, fully determined by `seed`.
pub fn pair_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(pair_deck(&mut GameRng::substream(seed, 32_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a pair deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    #[test]
    fn a_deck_has_sixty_cards_and_every_pair_card_and_the_corpus_cards_are_verified() {
        let deck = pair_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(deck.len(), 60);
        let corpus = corpus::corpus().unwrap();
        for &(name, copies) in PAIR_CARDS {
            assert_eq!(deck.iter().filter(|card| card.name == name).count(), copies);
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
