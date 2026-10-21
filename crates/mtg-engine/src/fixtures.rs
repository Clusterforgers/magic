//! Small game states for the unit tests of the engine itself (the testkit has the richer builders).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;

use crate::format::Normal;
use crate::setup::{GameSetup, PlayerSetup, PresetPermanent, Start};
use crate::state::GameState;
use crate::turn::Step;

/// A creature with a name and stats and no abilities.
pub(crate) fn creature(name: &str, power: i32, toughness: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, toughness),
    )
}

/// A two-player game starting at the first main step of turn 1: `hands[i]` and `battlefields[i]` are seat
/// `i`'s cards, and every library holds five vanilla creatures.
pub(crate) fn setup(
    hands: Vec<Vec<Arc<CardDefinition>>>,
    battlefields: Vec<Vec<Arc<CardDefinition>>>,
) -> GameSetup {
    let players = hands
        .into_iter()
        .zip(battlefields)
        .map(|(hand, battlefield)| PlayerSetup {
            library: (0..5)
                .map(|n| creature(&format!("Library {n}"), 1, 1))
                .collect(),
            hand,
            battlefield: battlefield.into_iter().map(PresetPermanent::new).collect(),
            graveyard: Vec::new(),
            life: None,
            keywords: Vec::new(),
            counters: Vec::new(),
        })
        .collect();
    GameSetup {
        format: Arc::new(Normal),
        house_rules: Default::default(),
        seed: 7,
        players,
        start: Start::At {
            turn: 1,
            active: PlayerId(0),
            step: Step::Main1,
        },
        validate_decks: false,
    }
}

/// The state [`setup`] begins in.
pub(crate) fn state(
    hands: Vec<Vec<Arc<CardDefinition>>>,
    battlefields: Vec<Vec<Arc<CardDefinition>>>,
) -> GameState {
    match GameState::from_setup(&setup(hands, battlefields)) {
        Ok(state) => state,
        Err(error) => panic!("the fixture setup is valid: {error}"),
    }
}
