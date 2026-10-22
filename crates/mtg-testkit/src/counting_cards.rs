//! Spells that cost less for what is counted (creatures, land cards in a graveyard, the total power of creatures, attacking
//! creatures), cost less when cast from anywhere other than a hand, and permanents whose group enters tapped, with the decks
//! and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus::{self, CorpusError};

/// Lands in a counting deck: five colours in turn.
const LANDS: usize = 26;

/// The corpus cards whose cost counts something or names where a spell is cast from, and those that make a group enter
/// tapped, which a game uses when they are cast.
pub const COUNTING_CARDS: &[&str] = &[
    "Vanquish the Horde",
    "Blasphemous Act",
    "Primeval Protector",
    "Rumbleweed",
    "Ghalta, Primal Hunger",
    "Nyxborn Behemoth",
    "Ancient Stone Idol",
    "Sage of the Beyond",
    "Authority of the Consuls",
    "Kinjalli's Sunwing",
    "Urabrask the Hidden",
];

/// A legal Normal deck in five colours of the cards above, one of each, spells to fill the graveyards and cheap creatures
/// and mana to cast them with (the counting spells cost twelve at most).
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn counting_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Green,
        Color::White,
        Color::Blue,
        Color::Red,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &name in COUNTING_CARDS {
        deck.push(corpus::card(name)?);
    }
    for (name, copies) in [
        ("Lightning Bolt", 4),
        ("Giant Growth", 4),
        ("Pacifism", 2),
        ("Faithless Looting", 2),
        ("Llanowar Elves", 4),
        ("Sol Ring", 3),
        ("Grizzly Bears", 4),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`counting_deck`]s, fully determined by `seed`.
pub fn counting_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(counting_deck(&mut GameRng::substream(seed, 16_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a counting deck is a legal sixty cards with every card of the list in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_counting_deck_is_sixty_cards_with_every_listed_card() {
        let deck = counting_deck(&mut GameRng::substream(1, 16_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in COUNTING_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = counting_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = counting_fuzz_setup(3, 5).unwrap();
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
