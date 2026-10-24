//! The tail of single blockers: the decks and games built from the corpus cards that the life gained this turn, a choice to
//! gain life, an Equipment that attaches as it enters, a prevention and "he connives" freed.

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

/// The corpus cards of the tail, with their copies in a deck.
pub const TAIL_CARDS: &[(&str, usize)] = &[
    ("Angelic Accord", 2),
    ("The Gaffer", 1),
    ("Griffin Aerie", 1),
    ("Gwaihir, Greatest of the Eagles", 1),
    ("Indulging Patrician", 1),
    ("Resplendent Angel", 1),
    ("Valkyrie Harbinger", 1),
    ("Auriok Champion", 1),
    ("Soul's Attendant", 2),
    ("Kang, Temporal Tyrant", 1),
    ("Ultron, Unlimited", 1),
    ("Maul of the Skyclaves", 1),
    ("Vibranium Strike Gauntlets", 1),
    ("Guard Gomazoa", 2),
    ("Seraph of the Sword", 2),
    ("Phantom Nantuko", 2),
    ("Phantom Nishoba", 2),
];

/// A legal Normal deck of sixty cards: the tail cards, life gain and burn for them to answer, and creatures, derived
/// entirely from `rng`.
pub fn tail_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in TAIL_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(salve("Tail Salve"), 3));
    deck.extend(std::iter::repeat_n(burn("Tail Burn", 2), 2));
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

/// A normal game of `players` seats whose decks are [`tail_deck`]s, fully determined by `seed`.
pub fn tail_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(tail_deck(&mut GameRng::substream(seed, 31_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a tail deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    #[test]
    fn a_deck_has_sixty_cards_and_every_tail_card_and_the_corpus_cards_are_verified() {
        let deck = tail_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(deck.len(), 60);
        let corpus = corpus::corpus().unwrap();
        for &(name, copies) in TAIL_CARDS {
            assert_eq!(deck.iter().filter(|card| card.name == name).count(), copies);
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
