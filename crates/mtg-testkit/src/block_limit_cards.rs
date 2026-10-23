//! Creatures that block only fliers, attackers that few creatures may block, and permanents that give melee and
//! myriad, with the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus::{self, CorpusError};

/// Lands in a block-limit deck: five colours in turn.
const LANDS: usize = 34;

/// The cards whose text limits who blocks or how many may block: they block only fliers, or are blocked by one
/// creature at most, by a group of them, or through an Aura or an Equipment.
pub const LIMITED_CARDS: &[&str] = &[
    "Cloud Sprite",
    "Shacklegeist",
    "Dense Canopy",
    "Air Bladder",
    "Stratus Walk",
    "Norwood Riders",
    "Challenger Troll",
    "Familiar Ground",
    "Rope",
    "Vorrac Battlehorns",
];

/// The cards that give melee or myriad to a group or to what they equip.
pub const KEYWORD_GIVERS: &[&str] = &[
    "Titania, Proud Pummeler",
    "Adriana, Captain of the Guard",
    "Skyhunter Strike Force",
    "Legion Loyalty",
    "Blade of Selves",
];

/// A legal sixty-card Normal deck in five colours of the cards above, one of each, with Bears, Giant Growth and
/// Lightning Bolt.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn block_limit_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &name in LIMITED_CARDS.iter().chain(KEYWORD_GIVERS) {
        deck.push(corpus::card(name)?);
    }
    for (name, copies) in [
        ("Grizzly Bears", 4),
        ("Giant Growth", 3),
        ("Lightning Bolt", 4),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`block_limit_deck`]s, fully determined by `seed`.
pub fn block_limit_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(block_limit_deck(&mut GameRng::substream(
            seed,
            38_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a block-limit deck is a legal sixty cards with every listed card in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_deck_is_sixty_cards_with_every_listed_card() {
        let deck = block_limit_deck(&mut GameRng::substream(1, 38_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in LIMITED_CARDS.iter().chain(KEYWORD_GIVERS) {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = block_limit_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = block_limit_fuzz_setup(3, 5).unwrap();
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
