//! Goal: shared helpers for the integration test files (this is not a test module itself).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};

/// Player id from a seat index.
pub fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

/// Names of the cards in `zone`, in zone order.
pub fn names(state: &GameState, zone: Zone) -> Vec<String> {
    state
        .zone(zone)
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|o| o.card().name.clone())
        .collect()
}

/// The card name of an object that still exists.
pub fn name_of(state: &GameState, id: mtg_core::ids::ObjectId) -> String {
    state
        .object(id)
        .map_or_else(|| "?".to_owned(), |o| o.card().name.clone())
}

/// One turn as seen in the log: who was active and which steps began.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnLog {
    pub active: PlayerId,
    pub steps: Vec<Step>,
}

/// Splits the log into turns; the opening turn is seeded with the step the scenario started in.
pub fn turn_logs(events: &[Event], start_active: PlayerId, start_step: Step) -> Vec<TurnLog> {
    let mut turns = vec![TurnLog {
        active: start_active,
        steps: vec![start_step],
    }];
    for event in events {
        match event {
            Event::TurnBegan { active, .. } => turns.push(TurnLog {
                active: *active,
                steps: Vec::new(),
            }),
            Event::StepBegan { step } => {
                if let Some(turn) = turns.last_mut() {
                    turn.steps.push(*step);
                }
            }
            _ => {}
        }
    }
    turns
}

/// Names of spells that resolved, in order.
pub fn resolved(state: &GameState, events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::SpellResolved { object } => Some(name_of(state, *object)),
            _ => None,
        })
        .collect()
}

/// The first battlefield object with this name.
pub fn on_battlefield(state: &GameState, name: &str) -> Option<mtg_core::ids::ObjectId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

/// A card of `card_type` for `{0}` whose one ability is a spell with these targets and effects, both written as JSON.
#[allow(clippy::expect_used)]
pub fn spell(
    name: &str,
    card_type: mtg_core::card::CardType,
    targets: &str,
    effects: &str,
) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let rules: mtg_cards::ability::Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"targets":{targets},"effects":{effects}}}}}"#
    ))
    .expect("the spell is written in the card DSL");
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new(name)
            .with_type(card_type)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(rules),
    )
}

pub mod combat;
pub mod forecast;
pub mod keywords;
pub mod removal;
pub mod seeds;
