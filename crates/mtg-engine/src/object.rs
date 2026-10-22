//! Game objects: cards, tokens and abilities on the stack, with their zone and permanent state.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::{CastWay, CounterKind};
use mtg_core::card::{CardType, CardTypes};
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::abilities::AbilityObject;
use crate::choose::Choices;
use crate::copies::CopyEffect;
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

/// Why an object lies face down (CR 708): the rules read that it does, who may look at it and what made it so. A zone
/// change clears it, because the object is then a new one (CR 400.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum FaceDown {
    /// A card foretold (CR 702.143a) on the turn with this number: it stays in exile and only its owner may look at it.
    Foretold {
        /// The number of the turn it was foretold in.
        turn: u32,
    },
}

impl FaceDown {
    /// Whether `viewer` may look at an object of `owner` that lies face down this way: the owner of a foretold card.
    pub fn may_look(self, owner: PlayerId, viewer: PlayerId) -> bool {
        match self {
            FaceDown::Foretold { .. } => owner == viewer,
        }
    }
}

/// The designations of a permanent: marks the rules put on it that it keeps only while it stays on the battlefield.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Designations {
    /// It has become monstrous (CR 701.37).
    pub(crate) monstrous: bool,
}

/// State that only exists while an object is on the battlefield.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermanentState {
    pub(crate) tapped: bool,
    /// The designations it has been given, which it keeps while it stays on the battlefield.
    pub(crate) designations: Designations,
    pub(crate) damage: u32,
    pub(crate) deathtouched: bool,
    pub(crate) summoning_sick: bool,
    /// The player whose turn began with it new to them: it had not been under their control since their previous turn
    /// began. Echo reads it in the upkeep (CR 702.30a), after the turn has cleared `summoning_sick`.
    pub(crate) new_to: Option<PlayerId>,
    pub(crate) counters: BTreeMap<CounterKind, u32>,
    pub(crate) attached_to: Option<ObjectRef>,
    /// The player it is attached to (an Aura that enchants a player, CR 303.4); never set together with `attached_to`.
    pub(crate) attached_player: Option<PlayerId>,
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
    /// The face of a card with several faces the permanent is (a modal double-faced card played as its back face).
    pub(crate) face: Option<u8>,
    /// The copy effect that makes the permanent a copy of another object (CR 707): its copiable values replace the card's.
    pub(crate) copy: Option<CopyEffect>,
}

impl PermanentState {
    /// The copy effect the permanent is under, if it is a copy of another object (CR 707).
    pub const fn copy(&self) -> Option<&CopyEffect> {
        self.copy.as_ref()
    }

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

    /// Whether the permanent has become monstrous (CR 701.37); a permanent that leaves the battlefield is a new
    /// object and is not.
    pub const fn monstrous(&self) -> bool {
        self.designations.monstrous
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

    /// Whether it was new to `player` when their current turn began: it had not been under their control since their
    /// previous turn began. What echo asks in the upkeep.
    pub fn is_new_to(&self, player: PlayerId) -> bool {
        self.new_to == Some(player)
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

    /// The player it is attached to (an Aura that enchants a player); `GameState::player_host_of` says whether they are still in the game.
    pub const fn attached_player(&self) -> Option<PlayerId> {
        self.attached_player
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
    /// Why the object lies face down, or `None` when it lies face up.
    pub(crate) face_down: Option<FaceDown>,
    /// The player who may cast the card from exile as the permanent card of an Adventure it was exiled "on" (CR 715.4); a zone change clears it.
    pub(crate) adventure: Option<PlayerId>,
    /// The spell was cast from its owner's hand (rebound reads it); a zone change clears it.
    pub(crate) cast_from_hand: bool,
    /// The spell was cast from a graveyard (aftermath reads it); a zone change clears it.
    pub(crate) cast_from_graveyard: bool,
    pub(crate) kind: ObjectKind,
    pub(crate) choices: Choices,
    pub(crate) ability: Option<Box<AbilityObject>>,
    /// The spell or ability on the stack this one is a copy of (CR 707.10).
    pub(crate) stack_copy: Option<ObjectRef>,
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
            face_down: None,
            adventure: None,
            cast_from_hand: false,
            cast_from_graveyard: false,
            kind: ObjectKind::Card,
            choices: Choices::default(),
            ability: None,
            stack_copy: None,
        }
    }

    /// An ability on top of the stack; it is named after its source and has no characteristics of its own.
    pub(crate) fn on_stack(
        id: ObjectId,
        controller: PlayerId,
        source: &GameObject,
        mut choices: Choices,
        ability: AbilityObject,
    ) -> Self {
        // Its source was in use as this face when the ability went on the stack, and the ability reads its name and types from it.
        choices.face = source.face();
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
            face_down: None,
            adventure: None,
            cast_from_hand: false,
            cast_from_graveyard: false,
            kind: ObjectKind::Ability,
            choices,
            ability: Some(Box::new(ability)),
            stack_copy: None,
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
            None if self.kind != ObjectKind::Ability && self.zone == Zone::Stack => {
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
            None if self.kind != ObjectKind::Ability && self.zone == Zone::Stack => {
                self.choices.way
            }
            None => None,
        }
    }

    /// The X this card was cast with: what the spell announced while it is on the stack and what the permanent
    /// remembers after; none elsewhere, where X is 0 (CR 107.3g, 202.3e).
    pub fn announced_x(&self) -> Option<u32> {
        match &self.permanent {
            Some(permanent) => permanent.x(),
            None if self.kind != ObjectKind::Ability && self.zone == Zone::Stack => self.choices.x,
            None => None,
        }
    }

    /// The spell or ability on the stack this object is a copy of, as it was when copied (CR 707.10); none for anything else.
    pub const fn copied_from(&self) -> Option<ObjectRef> {
        self.stack_copy
    }

    /// Whether this is a copy of a spell on the stack: a spell that is no card, and ceases to exist when it leaves the stack (CR 707.10).
    pub fn is_spell_copy(&self) -> bool {
        self.kind == ObjectKind::Token && self.zone == Zone::Stack
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

    /// The printed definition of the object: the face it is in use as (`face`), else the card as a whole.
    pub fn card(&self) -> &CardDefinition {
        if let Some(copy) = self.copy_effect() {
            return copy.definition();
        }
        match self.face() {
            Some(face) => self.def.face(face),
            None => &self.def,
        }
    }

    /// The whole card, with every face it has, whichever one is in use.
    pub fn whole_card(&self) -> &CardDefinition {
        match self.copy_effect() {
            Some(copy) => copy.definition(),
            None => &self.def,
        }
    }

    /// The copy effect the object is under, if it is a permanent that is a copy of another object (CR 707).
    pub fn copy_effect(&self) -> Option<&CopyEffect> {
        self.permanent.as_ref().and_then(PermanentState::copy)
    }

    /// The face of a card with several faces the object is in use as: the one chosen while it is a spell on the stack
    /// and a permanent, none elsewhere, where the card is its front face (or both halves of a split card).
    pub fn face(&self) -> Option<u8> {
        // Only a spell on the stack, an ability and a trial of a cast (`GameState::with_face`) hold a face in their choices.
        match &self.permanent {
            Some(permanent) => permanent.face,
            None => self.choices.face,
        }
    }

    /// Whether the spell was cast from its owner's hand.
    pub const fn cast_from_hand(&self) -> bool {
        self.cast_from_hand
    }

    /// Whether the spell was cast from a graveyard.
    pub const fn cast_from_graveyard(&self) -> bool {
        self.cast_from_graveyard
    }

    /// The player who may cast the card from exile as the creature of the Adventure it was exiled on.
    pub const fn on_adventure(&self) -> Option<PlayerId> {
        self.adventure
    }

    /// The printed card types of the face in use (no effects applied); a bestowed Aura is no creature (CR 702.103b).
    pub fn base_types(&self) -> CardTypes {
        let printed = if let Some(copy) = self.copy_effect() {
            copy.definition().type_set()
        } else {
            match self.face() {
                Some(face) if self.kind != ObjectKind::Ability => self.def.face(face).type_set(),
                _ => self.base_types,
            }
        };
        if self.bestowed() {
            crate::effects::without(printed, CardType::Creature)
        } else {
            printed
        }
    }

    /// Whether the object is bestowed: a spell cast for its bestow cost, or a permanent that was and is still attached to
    /// a permanent. It is an Aura with enchant creature then and no creature; a permanent that became unattached is a
    /// creature again (CR 702.103b, 702.103e).
    pub fn bestowed(&self) -> bool {
        match &self.permanent {
            Some(permanent) => {
                permanent.way == Some(CastWay::Bestow) && permanent.attached_to.is_some()
            }
            None => {
                self.kind != ObjectKind::Ability
                    && self.zone == Zone::Stack
                    && self.choices.way == Some(CastWay::Bestow)
            }
        }
    }

    /// The zone the object is in.
    pub const fn zone(&self) -> Zone {
        self.zone
    }

    /// How many counters of `kind` the card holds in exile (CR 702.62b: time counters of a suspended card).
    pub fn card_counters_of(&self, kind: &CounterKind) -> u32 {
        self.card_counters.get(kind).copied().unwrap_or(0)
    }

    /// Why the object lies face down, or `None` when it lies face up.
    pub const fn face_down(&self) -> Option<FaceDown> {
        self.face_down
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
            && self.face_down == other.face_down
            && self.adventure == other.adventure
            && self.cast_from_hand == other.cast_from_hand
            && self.cast_from_graveyard == other.cast_from_graveyard
            && self.counter == other.counter
            && self.kind == other.kind
            && self.choices == other.choices
            && self.ability == other.ability
            && self.stack_copy == other.stack_copy
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
        self.face_down.hash(state);
        self.adventure.hash(state);
        // Left out while it is off, so the hash of a game without rebound is the one it always had.
        if self.cast_from_hand {
            true.hash(state);
        }
        if self.cast_from_graveyard {
            "graveyard".hash(state);
        }
        self.counter.hash(state);
        self.kind.hash(state);
        self.choices.hash(state);
        self.ability.hash(state);
        // Left out while there is none, so the hash of a game without copies is the one it always had.
        if let Some(copied) = self.stack_copy {
            copied.hash(state);
        }
    }
}
