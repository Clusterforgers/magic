//! Creatures whose power and toughness count something (the cards in a hand or in graveyards, the card types among
//! them, the artifacts, the permanents of one colour, the nonbasic lands of the opponents) and the permanents that change
//! the life a player gains, with the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{land, salve};
use crate::corpus::{self, CorpusError};

/// Lands in a defining deck: five colours in turn.
const LANDS: usize = 26;

/// The names of the corpus cards whose characteristics a count defines or that change the life gained, which a game
/// uses when they enter the battlefield or are cast.
pub const DEFINING_CARDS: &[&str] = &[
    "Psychosis Crawler",
    "Lhurgoyf",
    "Nighthawk Scavenger",
    "Mortivore",
    "Master of Etherium",
    "Drove of Elves",
    "Haughty Djinn",
    "Wilderness Elemental",
    "Lord of Extinction",
    "Angel of Vitality",
    "Honor Troll",
    "Knight of Dusk's Shadow",
    "Giant Cindermaw",
    "Boon Reflection",
];

/// A legal Normal deck in five colours of the cards above, one of each, Lightning Bolt and Giant Growth to fill the graveyards,
/// Rampant Growth, Sol Ring and Salve (which gains life) and a Bear.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn defining_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &name in DEFINING_CARDS {
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
    deck.push(salve("Salve"));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`defining_deck`]s, fully determined by `seed`.
pub fn defining_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(defining_deck(&mut GameRng::substream(seed, 15_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a defining deck is a legal sixty cards with every card of the list in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_defining_deck_is_sixty_cards_with_every_listed_card() {
        let deck = defining_deck(&mut GameRng::substream(1, 15_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in DEFINING_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = defining_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = defining_fuzz_setup(3, 5).unwrap();
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
