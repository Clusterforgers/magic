//! Seeded random 60-card decks of basic lands and vanilla creatures for fuzz games.

use std::sync::Arc;

use mtg_core::card::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land};

/// Lands in a generated deck.
const LANDS: usize = 24;
/// Distinct creatures in a generated deck.
const DISTINCT_CREATURES: usize = 12;
/// Copies of each creature.
const COPIES: usize = 3;

/// A legal Normal deck in two colors, derived entirely from `rng`.
pub fn random_deck(rng: &mut GameRng) -> Vec<Arc<CardDefinition>> {
    let first = Color::ALL[rng.index(Color::ALL.len())];
    let second =
        Color::ALL[(first.index() + 1 + rng.index(Color::ALL.len() - 1)) % Color::ALL.len()];
    let colors = [first, second];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 2])).collect();
    for slot in 0..DISTINCT_CREATURES {
        let generic = u32::try_from(rng.index(4)).unwrap_or(0);
        let pips = 1 + rng.index(2);
        let cost_colors: Vec<Color> = (0..pips).map(|_| colors[rng.index(2)]).collect();
        let power = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(4)).unwrap_or(1);
        let name = format!("Fuzz {slot} {generic}+{pips} {power}/{toughness}");
        let card = creature(&name, generic, &cost_colors, power, toughness);
        deck.extend(std::iter::repeat_n(card, COPIES));
    }
    deck
}

/// A normal game of `players` seats with random decks, fully determined by `seed`.
pub fn fuzz_setup(players: usize, seed: u64) -> GameSetup {
    let decks = (0u64..)
        .take(players)
        .map(|seat| random_deck(&mut GameRng::substream(seed, 1_000 + seat)))
        .collect();
    GameSetup::new(Arc::new(Normal), seed, decks)
}

#[cfg(test)]
mod tests {
    //! Goal: generated decks are always legal Normal decks and are reproducible from the seed.
    use super::*;
    use mtg_engine::format::FormatRules;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn decks_are_legal_and_reproducible(seed in any::<u64>()) {
            let deck = random_deck(&mut GameRng::substream(seed, 0));
            prop_assert_eq!(deck.len(), 60);
            prop_assert!(Normal.validate_deck(&deck).is_ok());
            let again = random_deck(&mut GameRng::substream(seed, 0));
            prop_assert_eq!(deck, again);
        }
    }

    #[test]
    fn setups_have_one_deck_per_seat() {
        for players in [2, 3, 8] {
            let setup = fuzz_setup(players, 5);
            assert_eq!(setup.players.len(), players);
            assert!(setup.validate().is_ok());
        }
    }
}
