//! Myriad, squad and ravenous: the decks and games built from the corpus cards that have the keywords.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these keywords, one color after the other.
const LANDS: usize = 24;

/// The corpus cards with myriad.
pub const MYRIAD_CARDS: &[(&str, usize)] = &[
    ("Wyrm's Crossing Patrol", 3),
    ("Tiamat's Fanatics", 2),
    ("Dalek Squadron", 2),
    ("Tabaxi Toucaneers", 2),
    ("Herald of the Host", 2),
];

/// The corpus cards with squad.
pub const SQUAD_CARDS: &[(&str, usize)] = &[
    ("Powder Ganger", 2),
    ("Galadhrim Brigade", 2),
    ("Ultramarines Honour Guard", 2),
    ("Wasteland Raider", 2),
    ("Ruthless Radrat", 2),
    ("Vanguard Suppressor", 2),
];

/// The corpus cards with ravenous.
pub const RAVENOUS_CARDS: &[(&str, usize)] = &[
    ("Aberrant", 3),
    ("Jacked Rabbit", 2),
    ("Termagant Swarm", 2),
];

/// A legal Normal deck of sixty cards of the copy keywords, burn that kills the copies, and small creatures, derived
/// entirely from `rng`.
pub fn copy_keywords_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Red,
        Color::Black,
        Color::Green,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in MYRIAD_CARDS.iter().chain(SQUAD_CARDS).chain(RAVENOUS_CARDS) {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(burn("Burn", 2), 2));
    deck.extend(std::iter::repeat_n(
        free_creature("Mite", 1, 1),
        60 - deck.len(),
    ));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`copy_keywords_deck`]s, fully determined by `seed`.
pub fn copy_keywords_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(copy_keywords_deck(&mut GameRng::substream(
            seed,
            27_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a copy-keyword deck is a legal sixty cards with every card of the lists, fixed by its seed, and its corpus cards are verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_lists_and_is_fixed_by_its_seed() {
        let first = copy_keywords_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = copy_keywords_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        assert_ne!(
            names(&first),
            names(&copy_keywords_deck(&mut GameRng::substream(2, 0)).unwrap())
        );
        for &(name, copies) in MYRIAD_CARDS.iter().chain(SQUAD_CARDS).chain(RAVENOUS_CARDS) {
            let held = first.iter().filter(|card| card.name == name).count();
            assert_eq!(held, copies, "{name}");
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in MYRIAD_CARDS.iter().chain(SQUAD_CARDS).chain(RAVENOUS_CARDS) {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
