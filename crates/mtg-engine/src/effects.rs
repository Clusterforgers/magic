//! Continuous effects: what they change, which objects they touch and how long they last.

use mtg_core::card::CardType;
use mtg_core::color::ColorSet;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::Characteristics;
use crate::object::GameObject;
use crate::state::GameState;
use crate::zones::Zone;

/// A layer of the continuous-effect system (CR 613.1); effects apply in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Layer {
    /// Layer 1: copy effects.
    Copy,
    /// Layer 2: control-changing effects.
    Control,
    /// Layer 3: text-changing effects.
    Text,
    /// Layer 4: type-changing effects.
    Type,
    /// Layer 5: color-changing effects.
    Color,
    /// Layer 6: ability adding and removing effects.
    Ability,
    /// Layer 7a: characteristic-defining power and toughness.
    PtCharacteristicDefining,
    /// Layer 7b: effects that set power and toughness.
    PtSet,
    /// Layer 7c: effects that modify power and toughness (and counters).
    PtModify,
    /// Layer 7d: effects that switch power and toughness.
    PtSwitch,
}

/// Which objects an effect applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Selector {
    /// Exactly this object.
    Object(ObjectId),
    /// Every creature, judged by the characteristics built up to the effect's layer.
    AllCreatures,
    /// Every creature a player controls.
    CreaturesControlledBy(PlayerId),
}

impl Selector {
    /// Whether the effect touches `object` given its characteristics so far.
    pub(crate) fn matches(&self, object: &GameObject, so_far: &Characteristics) -> bool {
        match *self {
            Selector::Object(id) => object.id() == id,
            Selector::AllCreatures => so_far.types.contains(CardType::Creature),
            Selector::CreaturesControlledBy(player) => {
                so_far.types.contains(CardType::Creature) && object.controller() == player
            }
        }
    }
}

/// What an effect changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modification {
    /// Adds a card type (layer 4).
    AddType(CardType),
    /// Replaces the colors (layer 5).
    SetColors(ColorSet),
    /// Sets power and toughness (layer 7b).
    SetPowerToughness {
        /// New power.
        power: i32,
        /// New toughness.
        toughness: i32,
    },
    /// Adds to power and toughness (layer 7c).
    ModifyPowerToughness {
        /// Power delta.
        power: i32,
        /// Toughness delta.
        toughness: i32,
    },
}

impl Modification {
    /// The layer this kind of modification belongs to.
    pub const fn layer(&self) -> Layer {
        match self {
            Modification::AddType(_) => Layer::Type,
            Modification::SetColors(_) => Layer::Color,
            Modification::SetPowerToughness { .. } => Layer::PtSet,
            Modification::ModifyPowerToughness { .. } => Layer::PtModify,
        }
    }

    pub(crate) fn apply(&self, chars: &mut Characteristics) {
        match *self {
            Modification::AddType(ty) => chars.types = chars.types.with(ty),
            Modification::SetColors(colors) => chars.colors = colors,
            Modification::SetPowerToughness { power, toughness } => {
                chars.power = Some(power);
                chars.toughness = Some(toughness);
            }
            Modification::ModifyPowerToughness { power, toughness } => {
                chars.power = chars.power.map(|p| p + power);
                chars.toughness = chars.toughness.map(|t| t + toughness);
            }
        }
    }
}

/// How long an effect lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Duration {
    /// Until the cleanup step of the current turn (CR 514.2).
    UntilEndOfTurn,
    /// For as long as the source stays on the battlefield.
    WhileSourceOnBattlefield(ObjectId),
}

/// A registered continuous effect; characteristics are always derived from these.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContinuousEffect {
    /// Unique id.
    pub id: EffectId,
    /// Layer it applies in; always `modification.layer()`.
    pub layer: Layer,
    /// Affected objects.
    pub selector: Selector,
    /// The change.
    pub modification: Modification,
    /// How long it lasts.
    pub duration: Duration,
    /// Order of creation; later effects apply after earlier ones within a layer.
    pub timestamp: u64,
}

impl ContinuousEffect {
    /// Builds an effect, deriving its layer from the modification.
    pub fn new(
        id: EffectId,
        selector: Selector,
        modification: Modification,
        duration: Duration,
        timestamp: u64,
    ) -> Self {
        ContinuousEffect {
            id,
            layer: modification.layer(),
            selector,
            modification,
            duration,
            timestamp,
        }
    }

    /// Whether the duration has not yet ended in `state`.
    pub fn is_active(&self, state: &GameState) -> bool {
        match self.duration {
            Duration::UntilEndOfTurn => true,
            Duration::WhileSourceOnBattlefield(source) => state
                .object(source)
                .is_some_and(|o| o.zone() == Zone::Battlefield),
        }
    }
}
