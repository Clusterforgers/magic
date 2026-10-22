//! Devour, casualty and offspring: the decks and games built from the corpus cards that have the keywords.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these keywords, one color after the other.
const LANDS: usize = 22;

/// The corpus cards with devour, with their copies in a deck.
pub const DEVOUR_CARDS: &[(&str, usize)] = &[
    ("Gluttonous Slime", 2),
    ("Thunder-Thrash Elder", 2),
    ("Gorger Wurm", 1),
    ("Caprichrome", 1),
    ("Thorn-Thrash Viashino", 1),
    ("Caldera Hellion", 1),
];

/// The corpus cards with casualty, with their copies in a deck.
pub const CASUALTY_CARDS: &[(&str, usize)] = &[
    ("Light 'Em Up", 2),
    ("Join the Maestros", 2),
    ("Make Disappear", 1),
    ("Cut of the Profits", 1),
];

/// The corpus cards with offspring, with their copies in a deck.
pub const OFFSPRING_CARDS: &[(&str, usize)] = &[
    ("Steampath Charger", 2),
    ("Intrepid Rabbit", 2),
    ("Prosperous Bandit", 1),
    ("Rust-Shield Rampager", 1),
    ("Agate Instigator", 1),
];

/// The cards of all three lists.
fn keyword_cards() -> impl Iterator<Item = &'static (&'static str, usize)> {
    DEVOUR_CARDS
        .iter()
        .chain(CASUALTY_CARDS)
        .chain(OFFSPRING_CARDS)
}

/// A legal Normal deck of sixty cards of the three keywords, small creatures to sacrifice, creatures of power three that
/// can pay a casualty cost, and burn, derived entirely from `rng`.
pub fn devour_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Red,
        Color::Black,
        Color::Green,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in keyword_cards() {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(burn("Burn", 2), 2));
    // Four copies of a name at most: two small creatures to sacrifice, two of power three to pay casualty.
    let fodder = [("Mite", 1), ("Gnat", 1), ("Brute", 3), ("Ogre", 3)];
    for (name, power) in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(free_creature(name, *power, *power));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`devour_deck`]s, fully determined by `seed`.
pub fn devour_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(devour_deck(&mut GameRng::substream(seed, 28_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a devour deck is a legal sixty cards with every card of the lists, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_lists_and_is_fixed_by_its_seed() {
        let first = devour_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = devour_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        assert_ne!(
            names(&first),
            names(&devour_deck(&mut GameRng::substream(2, 0)).unwrap())
        );
        for &(name, copies) in keyword_cards() {
            let held = first.iter().filter(|card| card.name == name).count();
            assert_eq!(held, copies, "{name}");
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in keyword_cards() {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
