//! Vanishing, fading and backup: the corpus cards that have them, and the decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{burn, land};
use crate::corpus::{self, CorpusError};

/// Lands in a deck of these keywords, one color after the other.
const LANDS: usize = 20;

/// The corpus cards with vanishing, with their copies.
pub const VANISHING_CARDS: &[(&str, usize)] = &[
    ("Waning Wurm", 2),
    ("Calciderm", 1),
    ("Aven Riftwatcher", 1),
    ("Four Knocks", 1),
    ("Deep Forest Hermit", 1),
    ("Soultether Golem", 1),
    ("Lavacore Elemental", 1),
];

/// The corpus cards with fading, with their copies.
pub const FADING_CARDS: &[(&str, usize)] = &[
    ("Skyshroud Ridgeback", 2),
    ("Cloudskate", 1),
    ("Blastoderm", 2),
    ("Skyshroud Behemoth", 1),
    ("Rejuvenation Chamber", 1),
    ("Phyrexian Prowler", 1),
    ("Jolting Merfolk", 1),
    ("Woodripper", 1),
    ("Ancient Hydra", 1),
];

/// The corpus cards with backup, with their copies.
pub const BACKUP_CARDS: &[(&str, usize)] = &[
    ("Saiba Cryptomancer", 2),
    ("Chomping Kavu", 1),
    ("Consuming Aetherborn", 1),
    ("Serpent-Blade Assailant", 2),
    ("Bola Slinger", 1),
    ("Sigiled Sentinel", 1),
    ("Boon-Bringer Valkyrie", 1),
    ("Scorn-Blade Berserker", 1),
    ("Hangar Scrounger", 1),
    ("Redcap Heelslasher", 2),
    ("Golden-Scale Aeronaut", 1),
    ("Fearless Skald", 1),
    ("Death-Greeter's Champion", 1),
];

/// A legal Normal deck of sixty cards of the three keywords and burn to clear the way, derived entirely from `rng`.
pub fn lifecycle_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Black,
        Color::White,
        Color::Blue,
    ];
    let mut deck: Vec<Arc<CardDefinition>> =
        (0..LANDS).map(|i| land(colors[i % colors.len()])).collect();
    for &(name, copies) in VANISHING_CARDS
        .iter()
        .chain(FADING_CARDS)
        .chain(BACKUP_CARDS)
    {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    // Two names of burn, as a deck holds at most four copies of one card.
    let burn_cards = 60 - deck.len();
    deck.extend(std::iter::repeat_n(burn("Burn", 2), burn_cards / 2));
    deck.extend(std::iter::repeat_n(
        burn("Blaze", 3),
        burn_cards - burn_cards / 2,
    ));
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`lifecycle_deck`]s, fully determined by `seed`.
pub fn lifecycle_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(lifecycle_deck(&mut GameRng::substream(
            seed,
            27_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a lifecycle deck is a legal sixty cards with every card of the three lists, fixed by its seed, and every card of the lists is a Verified corpus card.
    use mtg_cards::support::Support;

    use super::*;

    fn names(deck: &[Arc<CardDefinition>]) -> Vec<String> {
        deck.iter().map(|card| card.name.clone()).collect()
    }

    #[test]
    fn a_deck_has_sixty_cards_and_every_card_of_the_lists_and_is_fixed_by_its_seed() {
        let first = lifecycle_deck(&mut GameRng::substream(1, 0)).unwrap();
        let again = lifecycle_deck(&mut GameRng::substream(1, 0)).unwrap();
        assert_eq!(first.len(), 60);
        assert_eq!(names(&first), names(&again));
        assert_ne!(
            names(&first),
            names(&lifecycle_deck(&mut GameRng::substream(2, 0)).unwrap())
        );
        for &(name, copies) in VANISHING_CARDS
            .iter()
            .chain(FADING_CARDS)
            .chain(BACKUP_CARDS)
        {
            let held = first.iter().filter(|card| card.name == name).count();
            assert_eq!(held, copies, "{name}");
        }
    }

    #[test]
    fn the_corpus_cards_of_the_deck_are_verified() {
        let corpus = corpus::corpus().unwrap();
        for &(name, _) in VANISHING_CARDS
            .iter()
            .chain(FADING_CARDS)
            .chain(BACKUP_CARDS)
        {
            assert_eq!(
                corpus.support(name).unwrap().support,
                Support::Verified,
                "{name}"
            );
        }
    }
}
