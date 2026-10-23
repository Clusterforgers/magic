//! Permanents whose static abilities give a group or a host keywords, a bonus or a base size (landwalk, fear, exalted,
//! ward, fabricate, annihilator, commander creatures, a subtype taken out, "during your turn"), with the decks and games
//! built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus::{self, CorpusError};

/// Lands in a grant deck: five colours in turn.
const LANDS: usize = 34;

/// The names of the corpus cards whose static abilities give a group or a host a keyword, a bonus or a base size.
pub const GRANT_CARDS: &[&str] = &[
    "Master of the Pearl Trident",
    "Merfolk Mistbinder",
    "Zombie Master",
    "Mask of Riddles",
    "Trace of Abundance",
    "Chains of Custody",
    "Elder Owyn Lyons",
    "Guardian Augmenter",
    "Bastion Protector",
    "Harmonious Archon",
    "Prava of the Steel Legion",
    "Eldrazi Conscription",
    "Cayth, Famed Mechanist",
    "Merchant of Truth",
    "Pontiff of Blight",
    "Tyranid Prime",
    "Titania, Nature's Force",
    "Nettlecyst",
    "All That Glitters",
    "Blazing Sunsteel",
    "Colossus Hammer",
    "Voice of All",
    "Riders of Gavony",
    "Hungry Lynx",
    "Lignify",
    "Kenrith's Transformation",
    "Darksteel Mutation",
    "Eye of Nidhogg",
];

/// A legal Normal deck in five colours of the cards above, one of each, with Lightning Bolt, Giant Growth and Bears.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn grant_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Black,
        Color::White,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    for &name in GRANT_CARDS {
        deck.push(corpus::card(name)?);
    }
    for (name, copies) in [
        ("Lightning Bolt", 3),
        ("Giant Growth", 3),
        ("Grizzly Bears", 3),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`grant_deck`]s, fully determined by `seed`.
pub fn grant_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(grant_deck(&mut GameRng::substream(seed, 17_000 + seat))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a grant deck is a legal sixty cards with every card of the list in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_grant_deck_is_sixty_cards_with_every_listed_card() {
        let deck = grant_deck(&mut GameRng::substream(1, 17_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in GRANT_CARDS {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = grant_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = grant_fuzz_setup(3, 5).unwrap();
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
