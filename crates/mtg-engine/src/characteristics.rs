//! Derived characteristics: base values from the card plus continuous effects, layer by layer.

use mtg_cards::AbilityDef;
use mtg_core::card::{CardType, CardTypes};
use mtg_core::color::ColorSet;
use mtg_core::ids::ObjectId;
use serde::{Deserialize, Serialize};

use crate::effects::ContinuousEffect;
use crate::object::ObjectKind;
use crate::state::GameState;
use crate::zones::Zone;

/// The characteristics of an object right now; never stored, always recomputed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Characteristics {
    /// Card types.
    pub types: CardTypes,
    /// Colors.
    pub colors: ColorSet,
    /// Power, if the object has one.
    pub power: Option<i32>,
    /// Toughness, if the object has one.
    pub toughness: Option<i32>,
    /// Abilities.
    pub abilities: Vec<AbilityDef>,
}

impl Characteristics {
    /// Whether the object is currently a creature.
    pub fn is_creature(&self) -> bool {
        self.types.contains(CardType::Creature)
    }
}

/// Computes the characteristics of `id` from its card and the active continuous effects (CR 613).
///
/// Effects apply layer by layer, and by timestamp within a layer; dependencies (CR 613.8) are not
/// modeled yet. Selectors are evaluated against the characteristics built up to their own layer.
/// Only objects on the battlefield are affected by effects. An ability on the stack has none.
pub fn characteristics(state: &GameState, id: ObjectId) -> Option<Characteristics> {
    let object = state.object(id)?;
    if object.kind() == ObjectKind::Ability {
        return Some(Characteristics {
            types: CardTypes::default(),
            colors: ColorSet::default(),
            power: None,
            toughness: None,
            abilities: Vec::new(),
        });
    }
    let card = object.card();
    let mut chars = Characteristics {
        types: object.base_types(),
        colors: card.colors(),
        power: card.power,
        toughness: card.toughness,
        abilities: card.abilities.clone(),
    };
    if object.zone() != Zone::Battlefield || state.effects().is_empty() {
        return Some(chars);
    }
    let mut active: Vec<&ContinuousEffect> = state
        .effects()
        .iter()
        .filter(|e| e.is_active(state))
        .collect();
    active.sort_by_key(|e| (e.layer, e.timestamp, e.id));
    for effect in active {
        if effect.selector.matches(object, &chars) {
            effect.modification.apply(&mut chars);
        }
    }
    Some(chars)
}
