//! Permanents whose static abilities act on a group (creatures that enter tapped, a keyword for the creatures with a
//! counter, ward for the other creatures) or skip a step, with the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus::{self, CorpusError};

/// Lands in a group-static deck: five colours in turn.
const LANDS: usize = 34;

/// The names of the corpus cards whose static abilities act on a group or skip a step.
pub const GROUP_STATIC_CARDS: &[&str] = &[
    "Authority of the Consuls",
    "Kinjalli's Sunwing",
    "Urabrask the Hidden",
    "Duskshell Crawler",
    "Pridemalkin",
    "Wild Wasteland",
    "Star Whale",
];

/// A legal Normal deck in five colours of the cards above, one of each, with Lightning Bolt (which asks for ward),
/// Giant Growth, Rampant Growth, Sol Ring, Bears and a Counterspell.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn group_static_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &name in GROUP_STATIC_CARDS {
        deck.push(corpus::card(name)?);
    }
    for (name, copies) in [
        ("Lightning Bolt", 4),
        ("Giant Growth", 4),
        ("Rampant Growth", 3),
        ("Sol Ring", 3),
        ("Grizzly Bears", 4),
        ("Counterspell", 1),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`group_static_deck`]s, fully determined by `seed`.
pub fn group_static_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(group_static_deck(&mut GameRng::substream(
            seed,
            16_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a group-static deck is a legal sixty cards with every card of the list in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_group_static_deck_is_sixty_cards_with_every_listed_card() {
        let deck = group_static_deck(&mut GameRng::substream(1, 16_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in GROUP_STATIC_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = group_static_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = group_static_fuzz_setup(3, 5).unwrap();
        let names = |setup: &GameSetup| {
            setup
                .players
                .iter()
                .map(|p| p.library.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&setup), names(&again));
    }
}
