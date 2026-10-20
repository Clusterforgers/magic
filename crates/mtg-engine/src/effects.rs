//! Continuous effects: what they change, which objects they touch and how long they last.

use mtg_cards::keyword::Keyword;
pub use mtg_cards::modify::Duration;
use mtg_core::card::CardType;
use mtg_core::color::ColorSet;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::Characteristics;
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;
use crate::turn::Step;
use crate::zones::Zone;

/// A layer of the continuous-effect system (CR 613.1); effects apply in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Selector {
    /// Exactly these objects, named when the effect began (CR 611.2c). One that has changed zones since is a new
    /// object (CR 400.7) and is not among them.
    Objects(Vec<ObjectRef>),
    /// Every creature, judged by the characteristics built up to the effect's layer.
    AllCreatures,
    /// Every creature a player controls.
    CreaturesControlledBy(PlayerId),
}

impl Selector {
    /// The objects with these ids as they are now; an id no object answers to is left out.
    pub fn objects(state: &GameState, ids: impl IntoIterator<Item = ObjectId>) -> Selector {
        Selector::Objects(
            ids.into_iter()
                .filter_map(|id| state.object(id).map(GameObject::reference))
                .collect(),
        )
    }

    /// Whether the effect touches `object` given its characteristics so far.
    pub(crate) fn matches(&self, object: &GameObject, so_far: &Characteristics) -> bool {
        match self {
            Selector::Objects(named) => named.contains(&object.reference()),
            Selector::AllCreatures => so_far.types.contains(CardType::Creature),
            Selector::CreaturesControlledBy(player) => {
                so_far.types.contains(CardType::Creature) && object.controller() == *player
            }
        }
    }
}

/// What an effect changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Modification {
    /// Adds a card type (layer 4).
    AddType(CardType),
    /// Replaces the colors (layer 5).
    SetColors(ColorSet),
    /// Adds a keyword ability (layer 6); `keywords_of` reads it and `characteristics` leaves it alone.
    AddKeyword(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Keyword),
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
            Modification::AddKeyword(_) => Layer::Ability,
            Modification::SetPowerToughness { .. } => Layer::PtSet,
            Modification::ModifyPowerToughness { .. } => Layer::PtModify,
        }
    }

    pub(crate) fn apply(&self, chars: &mut Characteristics) {
        match *self {
            Modification::AddType(ty) => chars.types = chars.types.with(ty),
            Modification::SetColors(colors) => chars.colors = colors,
            // Keywords are not part of `Characteristics`; `keywords_of` joins them to the printed ones.
            Modification::AddKeyword(_) => {}
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

/// A continuous effect from a resolved spell or ability (CR 611): characteristics are always derived from these.
///
/// An effect that came from a spell or ability names the objects it changes when it begins (CR 611.2c) and keeps
/// its place in the timestamp order for as long as it lasts (CR 613.7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    #[cfg_attr(
        feature = "ts",
        ts(
            type = "\"UntilEndOfTurn\" | \"UntilEndOfCombat\" | \"UntilYourNextTurn\" | \"WhileSourceOnBattlefield\" | \"Forever\""
        )
    )]
    pub duration: Duration,
    /// Order of creation; later effects apply after earlier ones within a layer.
    pub timestamp: u64,
    /// The object whose spell or ability created it, as it was then; `WhileSourceOnBattlefield` follows this object.
    pub source: Option<ObjectRef>,
    /// Who controlled that spell or ability: whose next turn `UntilYourNextTurn` waits for.
    pub controller: PlayerId,
    /// The number of the turn it began in, which `UntilEndOfCombat` and `UntilYourNextTurn` measure from.
    pub turn: u32,
}

impl ContinuousEffect {
    /// Builds an effect with no source, deriving its layer from the modification; it began in no turn and belongs to
    /// the first seat until [`begun`](Self::begun) says otherwise.
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
            source: None,
            controller: PlayerId(0),
            turn: 0,
        }
    }

    /// The same effect created by a spell or ability of `source`.
    #[must_use]
    pub fn from_source(mut self, source: ObjectRef) -> Self {
        self.source = Some(source);
        self
    }

    /// The same effect, begun in turn `turn` for `controller`.
    #[must_use]
    pub fn begun(mut self, controller: PlayerId, turn: u32) -> Self {
        self.controller = controller;
        self.turn = turn;
        self
    }

    /// Whether the effect still applies in `state`: its duration has not ended and an object it names is still the
    /// object it named. An effect that does not stops applying at once; the `EffectExpired` event only removes it.
    pub fn is_active(&self, state: &GameState) -> bool {
        let turn = state.turn();
        let lasts = match self.duration {
            // `UntilEndOfTurn` ends with the cleanup step, which logs the expiry (CR 514.2).
            Duration::UntilEndOfTurn | Duration::Forever => true,
            Duration::UntilEndOfCombat => {
                turn.number() == self.turn && turn.step() <= Step::EndCombat
            }
            Duration::UntilYourNextTurn => {
                state.is_alive(self.controller)
                    && !(turn.number() != self.turn && turn.active() == self.controller)
            }
            Duration::WhileSourceOnBattlefield => self.source.is_some_and(|source| {
                state
                    .object(source.id)
                    .is_some_and(|o| o.counter() == source.counter && o.zone() == Zone::Battlefield)
            }),
        };
        lasts && !self.spent(state)
    }

    /// Whether every object the effect names has become a new object or is gone, so it can never change anything again.
    fn spent(&self, state: &GameState) -> bool {
        match &self.selector {
            Selector::Objects(named) => named
                .iter()
                .all(|r| state.object(r.id).is_none_or(|o| o.counter() != r.counter)),
            Selector::AllCreatures | Selector::CreaturesControlledBy(_) => false,
        }
    }
}
