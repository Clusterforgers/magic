//! What a payment is for, and mana that may be spent only on some things ("Spend this mana only to cast creature spells").
//!
//! A restricted mana stays in the pool with what it allows until the pool empties. A payment may use it only for a
//! spell or an ability its restriction allows, and a source whose mana is restricted is a source of such a payment only.

use mtg_cards::spend::{ManaLasts, SpendOn, SpendTraits};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use serde::{Deserialize, Serialize};

use crate::characteristics::characteristics;
use crate::mana::ManaSource;
use crate::state::GameState;
use crate::zones::Zone;

/// One mana that may be spent only on what `only` lists.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Restricted {
    /// The mana.
    pub mana: ManaType,
    /// What it may be spent on: any one of these.
    pub only: Vec<SpendOn>,
    /// How long it stays in the pool; left out means until the step ends.
    #[serde(default, skip_serializing_if = "ManaLasts::is_step")]
    #[cfg_attr(feature = "ts", ts(as = "Option<ManaLasts>", optional))]
    pub lasts: ManaLasts,
}

/// What is paid for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum SpendKind {
    /// Neither a spell nor an ability: a cost an effect asks for ("unless its controller pays {2}").
    #[default]
    Other,
    /// Casting a spell.
    Spell,
    /// Activating an ability.
    Ability,
}

/// What a payment is for: the thing restricted mana is checked against.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Spend {
    /// A spell, an ability or neither.
    pub kind: SpendKind,
    /// The spell, or the source of the ability.
    pub traits: SpendTraits,
    /// The cost paid has {X} in it.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub has_x: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

impl Spend {
    /// A payment for neither a spell nor an ability: only mana without a restriction pays it.
    pub fn other() -> Self {
        Spend::default()
    }

    /// `caster` casting `spell`; `has_x` when its cost has {X}.
    pub fn for_spell(state: &GameState, spell: ObjectId, caster: PlayerId, has_x: bool) -> Self {
        let mut traits = traits_of(state, spell);
        if let Some(object) = state.object(spell) {
            traits.not_owned = object.owner() != caster;
            // A card on its way to the stack is still in its zone; one on the stack remembers where it came from.
            traits.from_graveyard = match object.zone() {
                Zone::Graveyard(_) => true,
                Zone::Stack => object.cast_from_graveyard(),
                _ => false,
            };
        }
        Spend {
            kind: SpendKind::Spell,
            traits,
            has_x,
        }
    }

    /// The payment for a creature card cast for its bestow cost: an Aura spell, no creature spell (CR 702.103b).
    #[must_use]
    pub fn bestowed(mut self) -> Self {
        let aura = mtg_core::card::Subtype("Aura".to_owned());
        self.traits.types =
            crate::effects::without(self.traits.types, mtg_core::card::CardType::Creature);
        if !self.traits.subtypes.contains(&aura) {
            self.traits.subtypes.push(aura);
        }
        self
    }

    /// The payment for a card cast face down: a creature spell with no supertype, subtype or colour (CR 708.4).
    #[must_use]
    pub fn face_down(mut self) -> Self {
        self.traits = SpendTraits::default();
        self.traits.types = self.traits.types.with(mtg_core::card::CardType::Creature);
        self
    }

    /// The payment for the spell as cast the way `way` says: a bestowed card is an Aura spell, a card cast face down a
    /// creature spell that is nothing else.
    #[must_use]
    pub fn cast_as(self, way: Option<mtg_cards::value::CastWay>) -> Self {
        match way {
            Some(mtg_cards::value::CastWay::Bestow) => self.bestowed(),
            Some(mtg_cards::value::CastWay::FaceDown) => self.face_down(),
            _ => self,
        }
    }

    /// Activating an ability of `source`; `has_x` when its cost has {X}.
    pub fn for_ability(state: &GameState, source: ObjectId, has_x: bool) -> Self {
        Spend {
            kind: SpendKind::Ability,
            traits: traits_of(state, source),
            has_x,
        }
    }

    /// Whether mana restricted to `only` may be spent on this; mana with no restriction may be spent on anything.
    pub fn allows(&self, only: &[SpendOn]) -> bool {
        only.is_empty()
            || only.iter().any(|on| match on {
                SpendOn::Spell(filter) => {
                    self.kind == SpendKind::Spell && filter.holds(&self.traits)
                }
                SpendOn::Ability(filter) => {
                    self.kind == SpendKind::Ability && filter.holds(&self.traits)
                }
                SpendOn::CostWithX => self.kind != SpendKind::Other && self.has_x,
            })
    }

    /// Whether the payment is for neither a spell nor an ability, so serializing can leave it out.
    pub const fn is_other(&self) -> bool {
        matches!(self.kind, SpendKind::Other)
    }
}

/// What a restriction reads of the object: its types, supertypes, subtypes and number of colors as the game sees it now.
fn traits_of(state: &GameState, id: ObjectId) -> SpendTraits {
    let (Some(object), Some(chars)) = (state.object(id), characteristics(state, id)) else {
        return SpendTraits::default();
    };
    let card = object.card();
    let mut subtypes: Vec<_> = card
        .subtypes
        .iter()
        .filter(|subtype| chars.has_subtype(&card.subtypes, subtype))
        .cloned()
        .collect();
    for subtype in chars.subtypes {
        if !subtypes.contains(&subtype) {
            subtypes.push(subtype);
        }
    }
    SpendTraits {
        types: chars.types,
        supertypes: card.supertypes.iter().copied().collect(),
        subtypes,
        colors: u8::try_from(chars.colors.iter().count()).unwrap_or(u8::MAX),
        value: object.mana_value(),
        x_in_cost: card.x_in_cost > 0,
        ..SpendTraits::default()
    }
}

/// One mana source of a payment as it is activated: the mana its own cost takes from the pool just before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The ability.
    pub source: ManaSource,
    /// The mana its cost takes from the pool; empty for a source that costs no mana.
    pub paid: Vec<ManaType>,
}

/// The mana a payment spent, as the events name it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Spent {
    /// All of it, by type.
    pub mana: Vec<ManaType>,
    /// The part of it that was restricted (each is also in `mana`).
    pub restricted: Vec<Restricted>,
    /// The sources the payment taps, in the order they are activated.
    pub steps: Vec<Step>,
}
