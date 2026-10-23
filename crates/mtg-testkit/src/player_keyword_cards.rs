//! Permanents that give their controller a keyword ("You have hexproof.", "You have shroud."), creatures that give
//! themselves or their friends a bonus and an ability in quotes under a condition, and creatures that enter with a
//! choice, with the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus::{self, CorpusError};

/// Lands in a player-keyword deck: five colours in turn.
const LANDS: usize = 42;

/// The cards that give their controller hexproof or shroud.
pub const PLAYER_KEYWORD_CARDS: &[&str] =
    &["Spirit of the Hearth", "Metropolis Reformer", "Ivory Mask"];

/// A legal sixty-card Normal deck in five colours of the cards above, two of each, with Bears, Giant Growth and
/// Lightning Bolt: the bolts are aimed at players, so the hexproof and shroud cards keep some of them out.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn player_keyword_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &name in PLAYER_KEYWORD_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, 2));
    }
    for (name, copies) in [
        ("Grizzly Bears", 4),
        ("Giant Growth", 4),
        ("Lightning Bolt", 4),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`player_keyword_deck`]s, fully determined by `seed`.
pub fn player_keyword_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(player_keyword_deck(&mut GameRng::substream(
            seed,
            39_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a player-keyword deck is a legal sixty cards with every listed card in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_deck_is_sixty_cards_with_every_listed_card() {
        let deck = player_keyword_deck(&mut GameRng::substream(1, 39_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in PLAYER_KEYWORD_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = player_keyword_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = player_keyword_fuzz_setup(3, 5).unwrap();
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
