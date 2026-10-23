//! Exile until a source leaves and mana spent to cast: the deck and games built from the corpus cards that have them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these cards, one color after the other.
const LANDS: usize = 24;

/// The corpus cards that exile until they leave the battlefield, with their copies in a deck.
pub const EXILE_UNTIL_CARDS: &[(&str, usize)] = &[
    ("Banisher Priest", 3),
    ("Banishing Light", 3),
    ("Cast Out", 2),
    ("Conclave Tribunal", 2),
    ("White Auracite", 2),
    ("Faith Unbroken", 2),
    ("Sheltered by Ghosts", 2),
    ("Angel of Sanctions", 2),
];

/// The corpus creatures that are sacrificed unless a mana of one color was spent to cast them, with their copies.
pub const SPENT_CARDS: &[(&str, usize)] = &[
    ("Azorius Herald", 3),
    ("Patagia Viper", 3),
    ("Plaxmanta", 3),
];

/// A legal Normal deck of sixty cards of both lists, burn that kills the exilers, and plain creatures, derived entirely
/// from `rng`.
pub fn exile_until_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Green,
        Color::Red,
        Color::Black,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in EXILE_UNTIL_CARDS.iter().chain(SPENT_CARDS) {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(burn("Burn", 3), 4));
    let fodder = [("Bear", 2), ("Ogre", 3), ("Troll", 4), ("Wisp", 1)];
    for (name, power) in fodder.iter().cycle().take(60 - deck.len()) {
        deck.push(free_creature(name, *power, *power));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`exile_until_deck`]s, fully determined by `seed`.
pub fn exile_until_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(exile_until_deck(&mut GameRng::substream(
            seed,
            38_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: an exile-until deck is a legal sixty cards with every card of both lists, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_lists_and_is_fixed_by_its_seed() {
        let first = exile_until_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = exile_until_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        for &(name, copies) in EXILE_UNTIL_CARDS.iter().chain(SPENT_CARDS) {
            assert_eq!(
                first.iter().filter(|card| card.name == name).count(),
                copies
            );
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in EXILE_UNTIL_CARDS.iter().chain(SPENT_CARDS) {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}: {:?}",
                corpus.support(name).unwrap()
            );
        }
    }
}
