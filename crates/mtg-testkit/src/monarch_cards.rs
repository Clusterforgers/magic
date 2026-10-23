//! The monarch cards: the deck and games built from the corpus cards that make a player the monarch or ask who is.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land, monarcher};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 24;

/// The corpus cards with a wording of the monarch beyond the designation, with their copies in a deck.
pub const MONARCH_CARDS: &[(&str, usize)] = &[
    ("Archon of Coronation", 3),
    ("Archivist of Gondor", 2),
    ("M'Baku, Jabari Chieftain", 3),
    ("Knights of the Black Rose", 3),
    ("Palace Jailer", 3),
    ("Regal Behemoth", 2),
    ("Custodi Lich", 2),
    ("Dawnglade Regent", 2),
];

/// A legal Normal deck of sixty cards of the list, instants that make their caster the monarch, burn and plain creatures,
/// derived entirely from `rng`.
pub fn monarch_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Green,
        Color::Red,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in MONARCH_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(monarcher("Crown"), 3));
    deck.extend(std::iter::repeat_n(burn("Burn", 3), 4));
    let fodder = [("Bear", 2), ("Ogre", 3), ("Troll", 4), ("Wisp", 1)];
    for (name, power) in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(free_creature(name, *power, *power));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`monarch_deck`]s, fully determined by `seed`.
pub fn monarch_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(monarch_deck(&mut GameRng::substream(seed, 39_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a monarch deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = monarch_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = monarch_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in MONARCH_CARDS {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in MONARCH_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}: {:?}",
                corpus.support(name).unwrap()
            );
        }
    }
}
