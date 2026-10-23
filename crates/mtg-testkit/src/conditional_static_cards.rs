//! Cards whose statics hold only under a condition (devotion, delirium, quest counters, an untapped body), with the
//! decks and games built from them.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::rng::GameRng;

use crate::cards::land;
use crate::corpus::{self, CorpusError};

/// Lands in a conditional-static deck: five colors in turn.
const LANDS: usize = 20;

/// The Gods, creatures only with enough devotion.
pub const GODS: &[&str] = &[
    "Thassa, God of the Sea",
    "Heliod, God of the Sun",
    "Erebos, Bleak-Hearted",
    "Karametra, God of Harvests",
];

/// The creatures that grow or gain a keyword with four card types in their controller's graveyard.
pub const DELIRIUM_CREATURES: &[&str] = &["Gnarlwood Dryad", "Grim Flayer", "Deathcap Cultivator"];

/// The enchantments that pump a team once they hold enough quest counters.
pub const QUESTS: &[&str] = &["Quest for the Goblin Lord", "Beastmaster Ascension"];

/// The other cards that are conditional statics: an untapped hexproof body, counters, a legendary host.
pub const OTHER_CONDITIONAL: &[&str] = &[
    "Paradise Druid",
    "Dragonlord Ojutai",
    "Voice of the Blessed",
    "Champion's Helm",
    "Hero's Heirloom",
];

/// A creature that costs two blue mana, to build devotion.
fn blue_creature(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse("{U}{U}").unwrap_or(ManaCost::ZERO)),
    )
}

/// A cheap artifact creature, to put a fourth card type into a graveyard.
fn scrap_golem() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Scrap Golem")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_mana_cost(ManaCost::parse("{2}").unwrap_or(ManaCost::ZERO)),
    )
}

/// A legal sixty-card Normal deck of the cards above with blue creatures, artifact creatures and the spells that fill a
/// graveyard.
///
/// It has its own generator, so the other decks and the games seeded from them stay as they were.
pub fn conditional_static_deck(rng: &mut GameRng) -> Result<Vec<Arc<CardDefinition>>, CorpusError> {
    let colors = [
        Color::Blue,
        Color::Blue,
        Color::Green,
        Color::White,
        Color::Red,
    ];
    let mut deck: Vec<Arc<CardDefinition>> = (0..LANDS).map(|i| land(colors[i % 5])).collect();
    deck.extend(std::iter::repeat_n(
        corpus::card("Thassa, God of the Sea")?,
        4,
    ));
    for &name in GODS
        .iter()
        .skip(1)
        .chain(DELIRIUM_CREATURES)
        .chain(QUESTS)
        .chain(OTHER_CONDITIONAL)
    {
        deck.push(corpus::card(name)?);
    }
    for name in ["Tide Weaver", "Wave Binder", "Reef Singer"] {
        deck.extend(std::iter::repeat_n(blue_creature(name), 4));
    }
    deck.extend(std::iter::repeat_n(scrap_golem(), 4));
    for (name, copies) in [
        ("Grizzly Bears", 2),
        ("Giant Growth", 1),
        ("Lightning Bolt", 3),
        ("Rampant Growth", 1),
    ] {
        deck.extend(std::iter::repeat_n(corpus::card(name)?, copies));
    }
    rng.shuffle(&mut deck);
    Ok(deck)
}

/// A normal game of `players` seats whose decks are [`conditional_static_deck`]s, fully determined by `seed`.
pub fn conditional_static_fuzz_setup(players: usize, seed: u64) -> Result<GameSetup, CorpusError> {
    let mut decks = Vec::with_capacity(players);
    for seat in 0u64..players as u64 {
        decks.push(conditional_static_deck(&mut GameRng::substream(
            seed,
            41_000 + seat,
        ))?);
    }
    Ok(GameSetup::new(Arc::new(Normal), seed, decks))
}

#[cfg(test)]
mod tests {
    //! Goal: a conditional-static deck is a legal sixty cards with every listed card in it, and the games built from one are fixed by their seed.
    use super::*;

    #[test]
    fn a_deck_is_sixty_cards_with_every_listed_card() {
        let deck = conditional_static_deck(&mut GameRng::substream(1, 41_000)).unwrap();
        assert_eq!(deck.len(), 60);
        for &name in GODS
            .iter()
            .chain(DELIRIUM_CREATURES)
            .chain(QUESTS)
            .chain(OTHER_CONDITIONAL)
        {
            assert!(deck.iter().any(|card| card.name == name), "{name}");
        }
        let setup = conditional_static_fuzz_setup(3, 5).unwrap();
        assert_eq!(setup.validate(), Ok(()));
        let again = conditional_static_fuzz_setup(3, 5).unwrap();
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
