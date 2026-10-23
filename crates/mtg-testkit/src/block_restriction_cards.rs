//! Creatures with block restrictions (less power can't block, blocked only by fliers or Slivers) and permanents that
//! grant mana abilities, with the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus::{self, CorpusError};

/// Lands in a block-restriction deck: five colours in turn.
const LANDS: usize = 34;

/// The creatures whose text restricts who can block them or whom they can block.
pub const RESTRICTED_CREATURES: &[&str] = &[
    "Aura Gnarlid",
    "Champion of Lambholt",
    "Silhana Ledgewalker",
    "Shifting Sliver",
    "Ant-Man, Elusive Avenger",
];

/// The permanents that give mana abilities to lands, creatures or Slivers.
pub const MANA_GRANTERS: &[&str] = &[
    "Chromatic Lantern",
    "Citanul Hierophants",
    "Gemhide Sliver",
    "Manaweft Sliver",
    "Abundant Growth",
    "Karametra's Favor",
    "Ninja Pizza",
];

/// A legal sixty-card Normal deck in five colours of the cards above, one of each, with Bears, Giant Growth and
/// Lightning Bolt.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn block_restriction_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &name in RESTRICTED_CREATURES.iter().chain(MANA_GRANTERS) {
        deck.push(corpus::card(name)?);
    }
    for (name, copies) in [
        ("Grizzly Bears", 4),
        ("Giant Growth", 4),
        ("Lightning Bolt", 4),
        ("Rampant Growth", 2),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`block_restriction_deck`]s, fully determined by `seed`.
pub fn block_restriction_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(block_restriction_deck(&mut GameRng::substream(
            seed,
            37_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a block-restriction deck is a legal sixty cards with every listed card in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_deck_is_sixty_cards_with_every_listed_card() {
        let deck = block_restriction_deck(&mut GameRng::substream(1, 37_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in RESTRICTED_CREATURES.iter().chain(MANA_GRANTERS) {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = block_restriction_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = block_restriction_fuzz_setup(3, 5).unwrap();
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
