//! Wire shapes of the per-viewer game state.
//!
//! These are the only game-state types that cross the wire. They hold no seed, RNG position or
//! library order, and an object appears only when its viewer may see which card it is.

use mtg_cards::AbilityDef;
use mtg_cards::ability::LoyaltyCost;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use mtg_engine::abilities::AbilityRef;
use mtg_engine::choose::Target;
use mtg_engine::combat::AttackTarget;
use mtg_engine::forecast::CombatForecast;
use mtg_engine::mana::ManaPool;
use mtg_engine::object::ObjectKind;
use mtg_engine::player::{LossReason, MulliganStatus};
use mtg_engine::state::{GameResult, Stage};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A zone: its size is always known, its cards only when the viewer may see them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ZoneView {
    /// Number of cards in the zone.
    pub count: u32,
    /// The cards in zone order (top last), or `None` when hidden from this viewer.
    pub cards: Option<Vec<ObjectId>>,
}

/// State that only exists while an object is on the battlefield.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PermanentView {
    /// Whether the permanent is tapped.
    pub tapped: bool,
    /// Damage marked on it this turn.
    pub damage: u32,
    /// Whether summoning sickness holds it back: it has not been under its controller's control since their turn began and has no haste.
    pub summoning_sick: bool,
}

/// A loyalty ability of a planeswalker and what activating it does to the loyalty counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LoyaltyAbilityView {
    /// Its address, which `PriorityOptions.abilities` lists while the holder of priority can activate it.
    pub ability: AbilityRef,
    /// Its cost: loyalty counters put on, removed, or neither.
    pub cost: LoyaltyCost,
}

/// A card the viewer may identify, with its current (layer-derived) characteristics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ObjectView {
    /// The object's id.
    pub id: ObjectId,
    /// The player who owns the card.
    pub owner: PlayerId,
    /// The player who controls it.
    pub controller: PlayerId,
    /// The zone it is in.
    pub zone: Zone,
    /// Printed name.
    pub name: String,
    /// Printed mana cost, if it has one.
    pub mana_cost: Option<ManaCost>,
    /// Printed supertypes.
    pub supertypes: Vec<Supertype>,
    /// Current card types.
    pub types: Vec<CardType>,
    /// Printed subtypes.
    pub subtypes: Vec<Subtype>,
    /// Current colors.
    pub colors: Vec<Color>,
    /// Current power, if it has one.
    pub power: Option<i32>,
    /// Current toughness, if it has one.
    pub toughness: Option<i32>,
    /// Current abilities.
    pub abilities: Vec<AbilityDef>,
    /// Names of the keywords it has now, such as `Flying` and `First strike`, in the order of its rules; left out while it has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub keywords: Vec<String>,
    /// Permanent state when on the battlefield.
    pub permanent: Option<PermanentView>,
    /// What the object is; a card when absent. An ability on the stack is named after its source and has no
    /// types, colours or abilities of its own.
    #[serde(default, skip_serializing_if = "ObjectKind::is_card")]
    #[ts(as = "Option<ObjectKind>", optional)]
    pub kind: ObjectKind,
    /// The object an ability on the stack comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<ObjectId>,
    /// What a spell or ability on the stack targets, one list for each target specification.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Vec<Target>>>", optional)]
    pub targets: Vec<Vec<Target>>,
    /// The counters on a permanent, by kind and number in a fixed order; left out while there are none. A planeswalker's
    /// loyalty is its loyalty counters, public like the rest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<(CounterKind, u32)>>", optional)]
    pub counters: Vec<(CounterKind, u32)>,
    /// The loyalty abilities of a planeswalker on the battlefield with their costs, in the order of its abilities; left
    /// out while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<LoyaltyAbilityView>>", optional)]
    pub loyalty_abilities: Vec<LoyaltyAbilityView>,
    /// The permanent this one is attached to, when it is an Aura or Equipment on the battlefield.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub attached_to: Option<ObjectId>,
    /// The permanents attached to this one, in id order; left out while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<ObjectId>>", optional)]
    pub attachments: Vec<ObjectId>,
    /// Whether the object lies face down (a foretold card in exile). The seat that may look at it sees its characteristics
    /// besides; every other seat sees an object with no name, types, colours, abilities or counters.
    #[serde(default, skip_serializing_if = "is_false")]
    #[ts(as = "Option<bool>", optional)]
    pub face_down: bool,
    /// The face of a card with several faces the object is in use as: the half cast, the Adventure, the back face of a
    /// modal double-faced card on the stack or the battlefield. Left out for the card as a whole, which is its front face
    /// or both halves of a split card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub face: Option<u8>,
}

/// Whether a flag is off, so serialization can leave it out (serde hands over a reference).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

/// One seat as a viewer sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PlayerView {
    /// The seat.
    pub seat: PlayerId,
    /// Life total.
    pub life: i32,
    /// Why the player lost, once they have.
    pub lost: Option<LossReason>,
    /// Mana floating in the pool.
    pub mana: ManaPool,
    /// The counters on the player (poison, energy, experience and the like), by kind in a fixed order; public, and left
    /// out while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<(CounterKind, u32)>>", optional)]
    pub counters: Vec<(CounterKind, u32)>,
    /// Mulligans taken.
    pub mulligans: u32,
    /// Opening-hand progress.
    pub mulligan: MulliganStatus,
    /// Hand; cards listed only for its owner, or for every seat when the table has open hands.
    pub hand: ZoneView,
    /// Library; the count only, never the cards but the top one while a rule or the table has it revealed to every seat.
    pub library: ZoneView,
    /// The top card of the library while a rule or the table has it revealed; left out otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub revealed_top: Option<ObjectId>,
    /// Graveyard (public).
    pub graveyard: ZoneView,
    /// Command zone (public).
    pub command: ZoneView,
}

/// Where the turn is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TurnView {
    /// Turn number, from 1.
    pub number: u32,
    /// The active player.
    pub active: PlayerId,
    /// The current step.
    pub step: Step,
    /// Lands the active player has played this turn.
    pub lands_played: u32,
}

/// Who holds priority and who has passed in this round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PriorityView {
    /// The player who holds priority.
    pub holder: PlayerId,
    /// Players who have passed since the last action.
    pub passed: Vec<PlayerId>,
}

/// An attacking creature and what blocks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AttackerView {
    /// The attacker.
    pub object: ObjectId,
    /// What it attacks.
    pub target: AttackTarget,
    /// Whether it has been blocked.
    pub blocked: bool,
    /// Creatures blocking it.
    pub blockers: Vec<ObjectId>,
}

/// Combat in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CombatView {
    /// Declared attackers.
    pub attackers: Vec<AttackerView>,
    /// Whether attackers have been declared this combat.
    pub attackers_declared: bool,
    /// Defending players who have declared blockers.
    pub blockers_declared: Vec<PlayerId>,
}

/// The table as a viewer sees it: everything but the objects themselves.
///
/// Every event batch carries a fresh one, so a client never derives any of it from events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct TableView {
    /// The seat this view was built for.
    pub viewer: PlayerId,
    /// Name of the format being played.
    pub format: String,
    /// What the table's creator set in place of the format's values; left out when nothing is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub house_rules: Option<HouseRules>,
    /// Coarse game stage.
    pub stage: Stage,
    /// The player who took the first turn.
    pub first_player: PlayerId,
    /// Turn position.
    pub turn: TurnView,
    /// Priority round, if one is under way.
    pub priority: Option<PriorityView>,
    /// All seats in seat order.
    pub players: Vec<PlayerView>,
    /// The shared battlefield.
    pub battlefield: ZoneView,
    /// The stack, top last.
    pub stack: ZoneView,
    /// The shared exile zone.
    pub exile: ZoneView,
    /// Combat in progress.
    pub combat: CombatView,
    /// What combat damage would do, while it is still ahead; derived from public state, so every seat sees the same.
    pub combat_forecast: Option<CombatForecast>,
    /// The result once the game is over.
    pub result: Option<GameResult>,
}

/// Everything one viewer may know about the game right now: the table plus every object they may identify.
///
/// On the wire the table fields and `objects` form one flat JSON object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct StateView {
    /// Everything but the objects.
    #[serde(flatten)]
    pub table: TableView,
    /// Every object the viewer may identify, in id order.
    pub objects: Vec<ObjectView>,
}
