//! Exalted, melee, mentor, dethrone, annihilator, extort and evolve: the corpus cards that have them, a synthetic
//! annihilator creature that is cheap enough to be cast, and the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, free_creature, keyworded_creature, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these keywords, one color after the other.
const LANDS: usize = 22;

/// The corpus cards of a triggered-keyword deck, with their copies.
pub const TRIGGERED_CARDS: &[(&str, usize)] = &[
    ("Akrasan Squire", 2),
    ("Aven Squire", 2),
    ("Sigiled Paladin", 1),
    ("Rhox Charger", 1),
    ("Basilica Screecher", 2),
    ("Tithe Drinker", 1),
    ("Syndic of Tithes", 2),
    ("Kingpin's Pet", 1),
    ("Cloudfin Raptor", 2),
    ("Crocanura", 2),
    ("Adaptive Snapjaw", 1),
    ("Battering Krasis", 1),
    ("Sunhome Stalwart", 2),
    ("Parhelion Patrol", 1),
    ("Hammer Dropper", 1),
    ("Wings of the Guard", 2),
    ("Deputized Protester", 1),
    ("Grenzo's Cutthroat", 2),
    ("Enraged Revolutionary", 2),
    ("Marchesa's Emissary", 1),
    ("Ulamog's Crusher", 1),
];

/// A creature for `{2}{R}` with annihilator 1, so that the keyword is met in games the corpus eldrazi are too dear for.
pub fn devourer() -> Arc<CardDefinition> {
    keyworded_creature(
        "Triggered Devourer",
        2,
        &[Color::Red],
        2,
        2,
        &[Keyword::Annihilator(1)],
    )
}

/// A legal Normal deck of sixty cards of the triggered keywords, burn and small creatures, derived entirely from `rng`.
pub fn triggered_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in TRIGGERED_CARDS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    deck.extend(std::iter::repeat_n(devourer(), 3));
    deck.extend(std::iter::repeat_n(free_creature("Mite", 1, 1), 2));
    deck.extend(std::iter::repeat_n(burn("Burn", 2), 2));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`triggered_deck`]s, fully determined by `seed`.
pub fn triggered_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(triggered_deck(&mut GameRng::substream(
            seed,
            27_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a triggered-keyword deck is a legal sixty cards with every card of the list, fixed by its seed, and every corpus card of it is Verified.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_list_and_is_fixed_by_its_seed() {
        let first = triggered_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = triggered_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        assert_ne!(
            names(&first),
            names(&triggered_deck(&mut GameRng::substream(2, 0)).unwrap())
        );
        for &(name, copies) in TRIGGERED_CARDS {
            let held = first.iter().filter(|card| card.name == name).count();
            assert_eq!(held, copies, "{name}");
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in TRIGGERED_CARDS {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
