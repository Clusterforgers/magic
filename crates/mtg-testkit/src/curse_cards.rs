//! The Curses that trigger on an attack at the enchanted player, and the decks and games built from them: the Curses of
//! Bounty, Chaos, Disturbance, Opulence, Shallow Graves, Verbosity and Vitality, the other two Curses of the corpus,
//! creatures to attack with and the burn that takes them away.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{bear, burn, elf, free_creature, giant, land};
use crate::corpus::{self, CorpusError};

/// Lands in a Curse deck: five colours in turn.
const LANDS: usize = 20;

/// The corpus Curses the deck holds, two copies of each (one of the two older ones).
pub const CURSE_CARDS: &[&str] = &[
    "Curse of Bounty",
    "Curse of Chaos",
    "Curse of Disturbance",
    "Curse of Opulence",
    "Curse of Shallow Graves",
    "Curse of Verbosity",
    "Curse of Vitality",
    "Curse of Predation",
    "Curse of the Forsaken",
];

/// A legal Normal deck of Curses: the corpus Curses, creatures to attack with and burn to take them away.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn curse_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Blue,
        Color::Red,
        Color::White,
        Color::Green,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for (index, name) in CURSE_CARDS.iter().enumerate() {
        let copies = if index < 7 { 2 } else { 1 };
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(burn("Spark", 2), 2));
    // Four copies of a name at most: creatures of six names fill the deck.
    let creatures = [
        bear(),
        elf(),
        giant(),
        free_creature("Raider", 3, 3),
        free_creature("Scout", 1, 1),
        free_creature("Brute", 2, 2),
    ];
    for creature in creatures.iter().cycle() {
        if deck.len() == 60 {
            break;
        }
        deck.push(Arc::clone(creature));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`curse_deck`]s, fully determined by `seed`.
pub fn curse_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(curse_deck(&mut GameRng::substream(seed, 23_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a Curse deck is a legal sixty cards with every Curse in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_curse_deck_is_sixty_cards_with_every_curse() {
        let deck = curse_deck(&mut GameRng::substream(1, 23_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for name in CURSE_CARDS {
            assert!(deck.iter().any(|card| card.name == *name), "{name}");
        }
        let setup = curse_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = curse_fuzz_setup(3, 5).unwrap();
        let names = |setup: &GameSetup| {
            setup
                .players
                .iter()
                .map(|p| p.library.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&setup), names(&again));
        assert_ne!(names(&setup), names(&curse_fuzz_setup(3, 6).unwrap()));
    }
}
