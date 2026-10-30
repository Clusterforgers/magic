//! Soulbond: the deck and games built from soulbond creatures and the cards that break a pair (a flicker, a change of
//! control, removal, an Aura that makes a creature a land).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{free_creature, keyworded_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 22;

/// The corpus cards of a soulbond deck with their copies: the creature that grants a trigger to both of a pair and the
/// cards that end a pair in each way.
pub const SOULBOND_CARDS: &[(&str, usize)] = &[
    ("Tandem Lookout", 4),
    ("Cloudshift", 3),
    ("Act of Treason", 2),
    ("Lightning Bolt", 3),
    ("Imprisoned in the Moon", 2),
    ("Control Magic", 1),
];

/// A legal Normal deck of sixty cards: the corpus cards of [`SOULBOND_CARDS`], soulbond creatures with nothing to give and
/// plain creatures to pair with, derived entirely from `rng`.
pub fn soulbond_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Blue,
        Color::White,
        Color::Red,
        Color::Blue,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in SOULBOND_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    let bonders = [("Bonder", 2, 2, 2), ("Pairing Giant", 4, 3, 3)];
    for (name, generic, power, toughness) in bonders {
        deck.extend(std::iter::repeat_n(
            keyworded_creature(name, generic, &[], power, toughness, &[Keyword::Soulbond]),
            3,
        ));
    }
    let fodder = [
        ("Mite", 1),
        ("Brute", 3),
        ("Imp", 2),
        ("Troll", 4),
        ("Wisp", 1),
        ("Drake", 2),
    ];
    for (name, power) in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(free_creature(name, *power, *power));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`soulbond_deck`]s, fully determined by `seed`.
pub fn soulbond_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(soulbond_deck(&mut GameRng::substream(seed, 31_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a soulbond deck is a legal sixty cards with every card of the list, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = soulbond_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = soulbond_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in SOULBOND_CARDS {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in SOULBOND_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
