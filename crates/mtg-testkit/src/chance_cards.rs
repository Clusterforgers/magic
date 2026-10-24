//! Repeating for each player, rolling dice and flipping coins: the deck and games built from the corpus cards that do it.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 24;

/// The corpus cards that repeat for each player, roll a die or flip a coin, with their copies in a deck.
pub const CHANCE_CARDS: &[(&str, usize)] = &[
    ("Incite Rebellion", 2),
    ("Treacherous Terrain", 2),
    ("Stormbreath Dragon", 2),
    ("Wound Reflection", 2),
    ("Archfiend of Despair", 2),
    ("Duskmantle Seer", 2),
    ("Mutalith Vortex Beast", 2),
    ("Boompile", 2),
    ("Goblin Archaeologist", 2),
    ("Earth-Cult Elemental", 2),
];

/// A legal Normal deck of sixty cards of the list and plain creatures, derived entirely from `rng`.
pub fn chance_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [Color::Red, Color::Black, Color::Blue, Color::Green];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in CHANCE_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    let mut slot = 0;
    while deck.len() < 60 {
        let power = 1 + slot % 4;
        let card = free_creature(&format!("Chance Fodder {slot}"), power, power);
        deck.extend(std::iter::repeat_n(card, 2.min(60 - deck.len())));
        slot += 1;
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`chance_deck`]s, fully determined by `seed`.
pub fn chance_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(chance_deck(&mut GameRng::substream(seed, 43_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a chance deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = chance_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = chance_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in CHANCE_CARDS {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_cards_of_the_list_are_verified() {
        for &(name, _) in CHANCE_CARDS {
            let record = corpus::corpus().unwrap().support(name).unwrap();
            assert_eq!(record.support, Support::Verified, "{name}");
        }
    }
}
