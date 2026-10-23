//! The decks and games of the cards that change a group or a target for a while: shrinks of everything but one creature
//! type, base sizes for a turn and until the next turn, lost abilities and a new look, grants to several targets, and
//! lands and artifacts that become creatures.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::{creature, land, wastes};
use crate::corpus::{self, CorpusError};

/// The corpus cards of a change deck, two copies each.
const SPELLS: [&str; 15] = [
    "Eyeblight Massacre",
    "Mass Diminish",
    "Flatline",
    "Turn to Frog",
    "Invisible Force Field",
    "Korozda Guildmage",
    "Llanowar Loamspeaker",
    "Liquimetal Torque",
    "Overwhelming Stampede",
    "Sudden Spoiling",
    "Shields of Velis Vel",
    "Nameless Inversion",
    "Biomass Mutation",
    "Olivia's Wrath",
    "Sage of the Maze",
];

/// Lands of a color in a change deck, in the order white, blue, black, green, then colorless ones.
const LANDS: [(Color, usize); 4] = [
    (Color::White, 5),
    (Color::Blue, 5),
    (Color::Black, 6),
    (Color::Green, 6),
];

/// A legal Normal deck of sixty cards: 24 lands, the [`SPELLS`] twice and six creatures of random sizes.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn change_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let mut deck: Vec<Arc<CardDefinition>> = Vec::with_capacity(60);
    for (color, copies) in LANDS {
        deck.extend(std::iter::repeat_n(land(color), copies));
    }
    deck.extend(std::iter::repeat_n(wastes(), 2));
    for name in SPELLS {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, 2));
    }
    let colors = [Color::White, Color::Blue, Color::Black, Color::Green];
    for slot in 0..6 {
        let power = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let toughness = i32::try_from(1 + rng.index(3)).unwrap_or(1);
        let name = format!("Change Fodder {slot} {power}/{toughness}");
        deck.push(creature(
            &name,
            1,
            &[colors[rng.index(4)]],
            power,
            toughness,
        ));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`change_deck`]s, fully determined by `seed`.
pub fn change_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(change_deck(&mut GameRng::substream(seed, 12_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a change deck is a legal sixty cards of corpus cards the engine plays whole.
    use mtg_engine::caps;

    use super::*;

    #[test]
    fn a_change_deck_has_sixty_cards() {
        let deck = change_deck(&mut GameRng::substream(1, 1)).unwrap();
        assert_eq!(deck.len(), 60);
    }

    #[test]
    fn every_spell_of_the_deck_uses_only_what_the_engine_runs() {
        let caps = caps();
        for name in SPELLS {
            let card = corpus::card(name).unwrap();
            for ability in &card.rules {
                for used in mtg_cards::validate::used_primitives(ability) {
                    assert!(caps.runs(&used), "{name}: {used}");
                }
            }
        }
    }
}
