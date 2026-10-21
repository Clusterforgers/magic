//! Game objects: cards, tokens and abilities on the stack, with their zone and permanent state.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::{CastWay, CounterKind};
use mtg_core::card::CardTypes;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::abilities::AbilityObject;
use crate::choose::Choices;
use crate::hash::stable_hash;
use crate::zones::Zone;

/// What a game object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ObjectKind {
    /// A card (a spell on the stack is its card).
    #[default]
    Card,
    /// A token.
    Token,
    /// An activated or triggered ability on the stack.
    Ability,
}

impl ObjectKind {
    /// Whether this is a card, so serialization can leave it out.
    pub const fn is_card(&self) -> bool {
        matches!(self, ObjectKind::Card)
    }
}

/// A reference to an object that notices when the object has become a new one by changing zones (CR 400.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ObjectRef {
    /// The object.
    pub id: ObjectId,
    /// Its zone-change counter when the reference was taken.
    pub counter: u32,
}

/// What the controller of a permanent chose as it entered the battlefield, which the permanent remembers for its
/// abilities to read (CR 614.1c).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Chosen {
    /// A colour.
    Color(mtg_core::color::Color),
    /// A creature type.
    CreatureType(mtg_core::card::Subtype),
}

/// State that only exists while an object is on the battlefield.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermanentState {
    pub(crate) tapped: bool,
    pub(crate) damage: u32,
    pub(crate) deathtouched: bool,
    pub(crate) summoning_sick: bool,
    pub(crate) counters: BTreeMap<CounterKind, u32>,
    pub(crate) attached_to: Option<ObjectRef>,
    pub(crate) timestamp: u64,
    pub(crate) observed: Option<PlayerId>,
    pub(crate) chosen: Option<Chosen>,
    /// The optional costs paid as the spell was cast, by index, a repeatable cost once for each payment.
    pub(crate) paid: Vec<usize>,
    /// The alternative cost the spell was cast for.
    pub(crate) way: Option<CastWay>,
    /// The X the spell was cast with, which its enters triggers and replacements read (CR 107.3m); the permanent
    /// itself has X = 0.
    pub(crate) x: Option<u32>,
}

impl PermanentState {
    /// The X the spell that became the permanent was cast with; its enters triggers and replacements read it (CR 107.3m).
    pub const fn x(&self) -> Option<u32> {
        self.x
    }

    /// The optional costs (kicker) paid as the spell that became the permanent was cast, by index, a repeatable cost
    /// once for each payment; its abilities read them.
    pub fn paid(&self) -> &[usize] {
        &self.paid
    }

    /// What its controller chose as it entered, if it asked for a choice.
    pub const fn chosen(&self) -> Option<&Chosen> {
        self.chosen.as_ref()
    }

    /// Whether the permanent is tapped.
    pub const fn tapped(&self) -> bool {
        self.tapped
    }

    /// Damage marked on the permanent this turn (tracked apart from toughness).
    pub const fn damage(&self) -> u32 {
        self.damage
    }

    /// Whether a source with deathtouch dealt it damage this turn (CR 704.5h).
    pub const fn deathtouched(&self) -> bool {
        self.deathtouched
    }

    /// Whether the controller has not yet controlled it since their turn began.
    pub const fn summoning_sick(&self) -> bool {
        self.summoning_sick
    }

    /// The counters on the permanent, by kind in a fixed order; a kind it has none of is not listed.
    pub fn counters(&self) -> impl Iterator<Item = (&CounterKind, u32)> {
        self.counters.iter().map(|(kind, count)| (kind, *count))
    }

    /// How many counters of `kind` are on the permanent.
    pub fn counters_of(&self, kind: &CounterKind) -> u32 {
        self.counters.get(kind).copied().unwrap_or(0)
    }

    /// The time the permanent entered the battlefield: the timestamp of its static abilities (CR 613.7).
    pub const fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Whether summoning sickness holds it back for `controller`: it has not been under their control since their turn
    /// began, because it entered since, or because it was under another player's control when it last was.
    pub fn sick_for(&self, controller: PlayerId) -> bool {
        self.summoning_sick || self.observed.is_some_and(|seen| seen != controller)
    }

    /// The permanent it is attached to, named as it was when attached (an Aura or Equipment); `GameState::host_of` says whether that is still so.
    pub const fn attached_to(&self) -> Option<ObjectRef> {
        self.attached_to
    }
}

/// A card, token or ability in the game with its current zone, controller and permanent state.
#[derive(Debug, Clone)]
pub struct GameObject {
    pub(crate) id: ObjectId,
    pub(crate) owner: PlayerId,
    pub(crate) controller: PlayerId,
    pub(crate) def: Arc<CardDefinition>,
    pub(crate) def_fingerprint: u64,
    pub(crate) base_types: CardTypes,
    pub(crate) zone: Zone,
    pub(crate) permanent: Option<PermanentState>,
    pub(crate) counter: u32,
    /// The counters on a card in exile (the time counters of a suspended card); a zone change clears them.
    pub(crate) card_counters: BTreeMap<CounterKind, u32>,
    pub(crate) kind: ObjectKind,
    pub(crate) choices: Choices,
    pub(crate) ability: Option<Box<AbilityObject>>,
}

impl GameObject {
    pub(crate) fn new(id: ObjectId, owner: PlayerId, def: Arc<CardDefinition>, zone: Zone) -> Self {
        GameObject {
            id,
            owner,
            controller: owner,
            def_fingerprint: stable_hash(&*def),
            base_types: def.type_set(),
            def,
            zone,
            permanent: None,
            counter: 0,
            card_counters: BTreeMap::new(),
            kind: ObjectKind::Card,
            choices: Choices::default(),
            ability: None,
        }
    }

    /// An ability on top of the stack; it is named after its source and has no characteristics of its own.
    pub(crate) fn on_stack(
        id: ObjectId,
        controller: PlayerId,
        source: &GameObject,
        choices: Choices,
        ability: AbilityObject,
    ) -> Self {
        GameObject {
            id,
            owner: controller,
            controller,
            def: Arc::clone(&source.def),
            def_fingerprint: source.def_fingerprint,
            base_types: CardTypes::default(),
            zone: Zone::Stack,
            permanent: None,
            counter: 0,
            card_counters: BTreeMap::new(),
            kind: ObjectKind::Ability,
            choices,
            ability: Some(Box::new(ability)),
        }
    }

    /// The object's id.
    pub const fn id(&self) -> ObjectId {
        self.id
    }

    /// The object's id and zone-change counter, for a reference that outlives a zone change.
    pub const fn reference(&self) -> ObjectRef {
        ObjectRef {
            id: self.id,
            counter: self.counter,
        }
    }

    /// How many times the object has changed zones: after a change it is a new object (CR 400.7).
    pub const fn counter(&self) -> u32 {
        self.counter
    }

    /// Whether this is a card, a token or an ability.
    pub const fn kind(&self) -> ObjectKind {
        self.kind
    }

    /// What was announced for the spell or ability while it is on the stack; empty elsewhere.
    pub const fn choices(&self) -> &Choices {
        &self.choices
    }

    /// The optional costs paid as this card was cast, by index: what the spell announced while it is on the stack and
    /// what the permanent remembers after; none elsewhere.
    pub fn paid_costs(&self) -> &[usize] {
        match &self.permanent {
            Some(permanent) => permanent.paid(),
            None if self.kind == ObjectKind::Card && self.zone == Zone::Stack => {
                &self.choices.optional_costs
            }
            None => &[],
        }
    }

    /// The alternative cost this card was cast for: what the spell announced while it is on the stack and what the
    /// permanent remembers after; none elsewhere.
    pub fn cast_way(&self) -> Option<CastWay> {
        match &self.permanent {
            Some(permanent) => permanent.way,
            None if self.kind == ObjectKind::Card && self.zone == Zone::Stack => self.choices.way,
            None => None,
        }
    }

    /// The X this card was cast with: what the spell announced while it is on the stack and what the permanent
    /// remembers after; none elsewhere, where X is 0 (CR 107.3g, 202.3e).
    pub fn announced_x(&self) -> Option<u32> {
        match &self.permanent {
            Some(permanent) => permanent.x(),
            None if self.kind == ObjectKind::Card && self.zone == Zone::Stack => self.choices.x,
            None => None,
        }
    }

    /// The ability this object is, if it is one on the stack.
    pub fn ability(&self) -> Option<&AbilityObject> {
        self.ability.as_deref()
    }

    /// The player who owns the card.
    pub const fn owner(&self) -> PlayerId {
        self.owner
    }

    /// The player who controls it (the owner unless on the battlefield or stack).
    pub const fn controller(&self) -> PlayerId {
        self.controller
    }

    /// The printed card definition.
    pub fn card(&self) -> &CardDefinition {
        &self.def
    }

    /// The printed card types (no effects applied).
    pub const fn base_types(&self) -> CardTypes {
        self.base_types
    }

    /// The zone the object is in.
    pub const fn zone(&self) -> Zone {
        self.zone
    }

    /// How many counters of `kind` the card holds in exile (CR 702.62b: time counters of a suspended card).
    pub fn card_counters_of(&self, kind: &CounterKind) -> u32 {
        self.card_counters.get(kind).copied().unwrap_or(0)
    }

    /// Permanent state when on the battlefield.
    pub const fn permanent(&self) -> Option<&PermanentState> {
        self.permanent.as_ref()
    }
}

impl PartialEq for GameObject {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.owner == other.owner
            && self.controller == other.controller
            && self.def_fingerprint == other.def_fingerprint
            && self.zone == other.zone
            && self.permanent == other.permanent
            && self.card_counters == other.card_counters
            && self.counter == other.counter
            && self.kind == other.kind
            && self.choices == other.choices
            && self.ability == other.ability
    }
}

impl Eq for GameObject {}

// The definition is hashed once at creation; hashing it per state hash would dominate runtime.
impl Hash for GameObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.owner.hash(state);
        self.controller.hash(state);
        self.def_fingerprint.hash(state);
        self.zone.hash(state);
        self.permanent.hash(state);
        self.card_counters.hash(state);
        self.counter.hash(state);
        self.kind.hash(state);
        self.choices.hash(state);
        self.ability.hash(state);
    }
}
