//! What a payment is for, and mana that may be spent only on some things ("Spend this mana only to cast creature spells").
//!
//! A restricted mana stays in the pool with what it allows until the pool empties. A payment may use it only for a
//! spell or an ability its restriction allows, and a source whose mana is restricted is a source of such a payment only.

use mtg_cards::spend::{ManaLasts, SpecialAction, SpendOn, SpendTraits};
use mtg_core::card::Subtype;
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
    /// Taking a special action of the kind (CR 116.2).
    Special(SpecialAction),
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

    /// A payment for the special action `action`: mana restricted to it or to other costs pays it.
    pub fn special(action: SpecialAction) -> Self {
        Spend {
            kind: SpendKind::Special(action),
            ..Spend::default()
        }
    }

    /// `caster` casting `spell`; `has_x` when its cost has {X}.
    pub fn for_spell(state: &GameState, spell: ObjectId, caster: PlayerId, has_x: bool) -> Self {
        let mut traits = traits_of(state, spell);
        if let Some(object) = state.object(spell) {
            traits.not_owned = object.owner() != caster;
            traits.shares_commander_type = shares_commander_type(state, spell, caster);
            traits.your_commander = state.is_commander(spell) && object.owner() == caster;
            // A card on its way to the stack is still in its zone; one on the stack remembers where it came from.
            traits.from_graveyard = match object.zone() {
                Zone::Graveyard(_) => true,
                Zone::Stack => object.cast_from_graveyard(),
                _ => false,
            };
            traits.from_exile = match object.zone() {
                Zone::Exile => true,
                Zone::Stack => object
                    .cast_record()
                    .is_some_and(|(zone, _)| zone == mtg_cards::refs::ZoneKind::Exile),
                _ => false,
            };
            traits.not_from_hand = match object.zone() {
                Zone::Hand(_) => false,
                Zone::Stack => !object.cast_from_hand(),
                _ => true,
            };
            // A spell on the stack remembers its kicker; one being announced learns it from its choices.
            traits.kicked = object.zone() == Zone::Stack
                && crate::announce::payments(
                    object.card(),
                    object.paid_costs(),
                    crate::keywords::OptionalKind::Kicker,
                ) > 0;
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
        self.traits.face_down = true;
        self
    }

    /// The payment for the spell as cast the way `way` says: a bestowed card is an Aura spell, a card cast face down a
    /// creature spell that is nothing else.
    #[must_use]
    pub fn cast_as(self, way: Option<mtg_cards::value::CastWay>) -> Self {
        match way {
            Some(mtg_cards::value::CastWay::Bestow) => self.bestowed(),
            Some(mtg_cards::value::CastWay::FaceDown) => self.face_down(),
            Some(mtg_cards::value::CastWay::Disturb) => {
                let mut spend = self;
                spend.traits.disturb = true;
                spend
            }
            _ => self,
        }
    }

    /// Activating an ability of `source`; `has_x` when its cost has {X}.
    pub fn for_ability(state: &GameState, source: ObjectId, has_x: bool) -> Self {
        let mut traits = traits_of(state, source);
        traits.on_battlefield = state
            .object(source)
            .is_some_and(|object| matches!(object.zone(), Zone::Battlefield));
        Spend {
            kind: SpendKind::Ability,
            traits,
            has_x,
        }
    }

    /// The payment for the ability `rules` at `address`, which is an equip ability when equip adds it (CR 702.6).
    #[must_use]
    pub fn of_ability(
        mut self,
        state: &GameState,
        address: crate::abilities::AbilityRef,
        rules: &mtg_cards::ability::Ability,
    ) -> Self {
        self.traits.equip = crate::activation_kind::specific(state, address, rules)
            == Some(mtg_cards::ability::ActivationKind::Equip);
        self
    }

    /// Whether mana restricted to `only` may be spent on this; mana with no restriction, or with a rider alone, may be
    /// spent on anything.
    pub fn allows(&self, only: &[SpendOn]) -> bool {
        !only.iter().any(SpendOn::restricts)
            || only.iter().any(|on| match on {
                SpendOn::Spell(filter) => {
                    self.kind == SpendKind::Spell && filter.holds(&self.traits)
                }
                SpendOn::Ability(filter) => {
                    self.kind == SpendKind::Ability && filter.holds(&self.traits)
                }
                SpendOn::CostWithX => self.kind != SpendKind::Other && self.has_x,
                // A special action is paid as a cost that is neither a spell's nor an ability's.
                SpendOn::OtherCosts => {
                    matches!(self.kind, SpendKind::Other | SpendKind::Special(_))
                }
                SpendOn::Special(action) => self.kind == SpendKind::Special(*action),
                // A rider limits nothing, so it neither allows nor refuses a payment.
                SpendOn::When(_)
                | SpendOn::Uncounterable(_)
                | SpendOn::Snow
                | SpendOn::Enters(_) => false,
            })
    }

    /// Whether the payment is for neither a spell nor an ability, so serializing can leave it out.
    pub const fn is_other(&self) -> bool {
        matches!(self.kind, SpendKind::Other)
    }
}

/// The kind of a mana source that a filter of sources reads: its types, supertypes and subtypes as the game sees it now.
pub(crate) fn source_kind(state: &GameState, id: ObjectId) -> SpendTraits {
    let full = traits_of(state, id);
    SpendTraits {
        types: full.types,
        supertypes: full.supertypes,
        subtypes: full.subtypes,
        ..SpendTraits::default()
    }
}

/// What a restriction reads of the object: its types, supertypes, subtypes and number of colors as the game sees it now.
pub(crate) fn traits_of(state: &GameState, id: ObjectId) -> SpendTraits {
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
    let keywords = crate::keywords::keywords_of(state, id);
    SpendTraits {
        types: chars.types,
        supertypes: card.supertypes.iter().copied().collect(),
        subtypes,
        colors: u8::try_from(chars.colors.iter().count()).unwrap_or(u8::MAX),
        color_set: chars.colors,
        value: object.mana_value(),
        x_in_cost: card.x_in_cost > 0,
        devoid: keywords.contains(&&mtg_cards::keyword::Keyword::Devoid),
        flashback: keywords
            .iter()
            .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Flashback(_))),
        freerunning: keywords
            .iter()
            .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Freerunning(_))),
        no_abilities: keywords.is_empty()
            && chars.abilities.is_empty()
            && crate::ability_reader::abilities_of(state, id).is_empty()
            && card.manual.is_empty(),
        ..SpendTraits::default()
    }
}

/// The creature types `id` has now, and whether it has every one (a changeling).
fn creature_types(state: &GameState, id: ObjectId) -> Option<(Vec<Subtype>, bool)> {
    let (object, chars) = (state.object(id)?, characteristics(state, id)?);
    let card = object.card();
    let mut types: Vec<Subtype> = Vec::new();
    let printed = card
        .subtypes
        .iter()
        .filter(|subtype| chars.has_subtype(&card.subtypes, subtype));
    for subtype in chars.subtypes.iter().chain(printed) {
        if subtype.is_creature_type() && !types.contains(subtype) {
            types.push(subtype.clone());
        }
    }
    Some((types, chars.all_creature_types))
}

/// Whether `spell` shares a creature type with a commander `caster` owns: a changeling shares one with anything that has
/// a creature type, and a commander with none (or none at all) shares nothing.
fn shares_commander_type(state: &GameState, spell: ObjectId, caster: PlayerId) -> bool {
    // Every spell asks, so the work waits for a commander and for a spell with a creature type.
    let mut commanders = state
        .commanders()
        .filter(|id| state.object(*id).is_some_and(|o| o.owner() == caster))
        .peekable();
    if commanders.peek().is_none() {
        return false;
    }
    let Some((own, own_all)) =
        creature_types(state, spell).filter(|(own, all)| *all || !own.is_empty())
    else {
        return false;
    };
    commanders
        .filter_map(|id| creature_types(state, id))
        .any(|(theirs, their_all)| {
            theirs.iter().any(|subtype| own.contains(subtype))
                || their_all
                || (own_all && !theirs.is_empty())
        })
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

/// The rider a snow source's mana carries (CR 107.4h): it pays for `{S}` and counts as snow mana spent.
pub(crate) fn snow_rider(state: &GameState, source: ObjectId) -> Option<SpendOn> {
    crate::characteristics::has_supertype(state, source, mtg_core::card::Supertype::Snow)
        .then_some(SpendOn::Snow)
}
