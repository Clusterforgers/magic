//! Cards that move cards between zones with a count or a counter: the deck and games built from the corpus spells that
//! return a card with a counter, return several targets to their owners' hands and exile a monocolored permanent.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 24;

/// The corpus spells that move cards by a count, a counter or a color, with their copies in a deck.
pub const MOVE_CARDS: &[(&str, usize)] =
    &[("Persist", 3), ("Aether Gale", 2), ("Vanishing Verse", 3)];

/// A legal Normal deck of sixty cards: the lands, the spells of [`MOVE_CARDS`], burn that fills the graveyards and
/// plain creatures, derived entirely from `rng`.
pub fn move_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Green,
        Color::Red,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in MOVE_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(burn("Burn", 3), 3));
    // Four copies of a name at most: the creatures come in groups of four with one name and one color, from 1/1 to 4/4,
    // so that a monocolored permanent is always there to exile.
    for at in 0..60 - deck.len() {
        let power = i32::try_from(at % 4).unwrap_or(0) + 1;
        let color = colors[(at / 4) % colors.len()];
        deck.push(creature(
            &format!("Fodder {}", at / 4),
            0,
            &[color],
            power,
            power,
        ));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`move_deck`]s, fully determined by `seed`.
pub fn move_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(move_deck(&mut GameRng::substream(seed, 41_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a move deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = move_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = move_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        let names = |deck: &[Arc<CardDefinition>]| -> Vec<String> {
            deck.iter().map(|card| card.name.clone()).collect()
        };
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in MOVE_CARDS {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in MOVE_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}: {:?}",
                corpus.support(name).unwrap()
            );
        }
    }
}
