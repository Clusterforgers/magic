//! Spells and permanents whose cost reduction depends on a condition (a Dragon, a flier, the permanents or creature cards
//! of the table, the basic lands of the opponents, life gained this turn) or names spells of two kinds, with the decks and
//! games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus::{self, CorpusError};

/// Lands in a reduction deck: five colours in turn.
const LANDS: usize = 26;

/// The corpus cards whose cost falls under a condition or for spells of two kinds.
pub const REDUCTION_CARDS: &[&str] = &[
    "Draconic Lore",
    "Winged Words",
    "Hour of Revelation",
    "Avatar of Woe",
    "Hagra Mauling // Hagra Broodpit",
    "Mortality Spear",
    "Stonybrook Banneret",
    "Frogtosser Banneret",
    "Nightscape Familiar",
    "Herald of War",
];

/// A legal Normal deck in five colours of the cards above, one of each, with the creatures and spells they reduce, a
/// spell that gains life and mana to cast them with.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn reduction_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Blue,
        Color::Black,
        Color::Green,
        Color::White,
        Color::Red,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &name in REDUCTION_CARDS {
        deck.push(corpus::card(name)?);
    }
    for (name, copies) in [
        ("Renewed Faith", 3),
        ("Atlantean Skirmisher", 3),
        ("Academy Elite", 2),
        ("Aven Squire", 3),
        ("Bogardan Hellkite", 1),
        ("Lightning Bolt", 3),
        ("Grizzly Bears", 4),
        ("Llanowar Elves", 3),
        ("Sol Ring", 3),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`reduction_deck`]s, fully determined by `seed`.
pub fn reduction_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(reduction_deck(&mut GameRng::substream(
            seed,
            17_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a reduction deck is a legal sixty-one cards with every card of the list in it, and the games built from one
    //! are fixed by their seed.
    use super::*;

    #[test]
    fn a_reduction_deck_has_every_listed_card_and_is_legal() {
        let deck = reduction_deck(&mut GameRng::substream(1, 17_000)).unwrap();
        for &name in REDUCTION_CARDS {
            // A card with two faces is named after its front face.
            let front = name.split(" // ").next().unwrap_or(name);
            assert!(deck.iter().any(|card| card.name == front), "{name}");
        }
        let setup = reduction_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let names = |setup: &GameSetup| {
            setup
                .players
                .iter()
                .map(|p| p.library.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&setup), names(&reduction_fuzz_setup(3, 5).unwrap()));
    }
}
