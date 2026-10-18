//! Decisions the engine asks for and the answers it accepts.
//!
//! A [`Decision`] lists every legal option; an [`Answer`] must be built from exactly those.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use serde::{Deserialize, Serialize};

use crate::combat::AttackTarget;
use crate::mana::{ManaOption, ManaPool, ManaSource, Payment};

/// A question for one player together with all of their legal options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Decision {
    /// Who must answer.
    pub player: PlayerId,
    /// What is asked, with the legal options inside.
    pub kind: DecisionKind,
}

/// The kind of decision, each variant carrying its exhaustive options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DecisionKind {
    /// Keep the opening hand or take a mulligan.
    Mulligan(MulliganOptions),
    /// Choose cards to put on the bottom of the library after keeping.
    BottomCards(BottomOptions),
    /// Act with priority or pass.
    Priority(PriorityOptions),
    /// Pay the mana cost of the spell being cast.
    PayMana(PayOptions),
    /// Declare attackers.
    DeclareAttackers(AttackOptions),
    /// Declare blockers.
    DeclareBlockers(BlockOptions),
    /// Divide an attacker's combat damage among its blockers.
    AssignDamage(DamageOptions),
    /// Discard down to maximum hand size.
    Discard(DiscardOptions),
}

impl DecisionKind {
    /// Short name used in error messages.
    pub const fn name(&self) -> &'static str {
        match self {
            DecisionKind::Mulligan(_) => "mulligan",
            DecisionKind::BottomCards(_) => "bottom-cards",
            DecisionKind::Priority(_) => "priority",
            DecisionKind::PayMana(_) => "pay-mana",
            DecisionKind::DeclareAttackers(_) => "declare-attackers",
            DecisionKind::DeclareBlockers(_) => "declare-blockers",
            DecisionKind::AssignDamage(_) => "assign-damage",
            DecisionKind::Discard(_) => "discard",
        }
    }
}

/// Options for the keep-or-mulligan decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MulliganOptions {
    /// The hand being judged.
    pub hand: Vec<ObjectId>,
    /// Mulligans already taken.
    pub mulligans_taken: u32,
    /// Whether another mulligan is allowed.
    pub can_mulligan: bool,
    /// Mulligans this format does not charge a card for (CR 103.5c gives Commander one).
    pub free_mulligans: u32,
    /// Cards put on the bottom if the hand is kept now: the count the bottom decision will ask for.
    pub bottom_if_keep: usize,
    /// Cards put on the bottom if the hand drawn by one more mulligan is kept (equals `bottom_if_keep` when it is free).
    pub bottom_if_mulligan: usize,
}

/// Options for choosing cards to bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BottomOptions {
    /// Cards in hand to choose from.
    pub hand: Vec<ObjectId>,
    /// Exactly this many must be chosen.
    pub count: usize,
}

/// Everything a player can do while holding priority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PriorityOptions {
    /// Lands that can be played now.
    pub playable_lands: Vec<ObjectId>,
    /// Spells that can be cast now with the mana available.
    pub castable: Vec<ObjectId>,
    /// Mana abilities that can be activated now.
    pub mana_abilities: Vec<ManaOption>,
    /// Passing is always legal.
    pub can_pass: bool,
}

/// Options while paying for a spell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PayOptions {
    /// The spell being cast.
    pub spell: ObjectId,
    /// The cost to pay.
    pub cost: ManaCost,
    /// Floating mana that may be spent.
    pub pool: ManaPool,
    /// Mana abilities that may be tapped.
    pub sources: Vec<ManaOption>,
    /// Whether an automatic payment exists (it always does when the cast was offered).
    pub auto_available: bool,
    /// Whether the cast may be backed out of; nothing has been paid yet, so it rolls back (CR 601.2).
    pub can_cancel: bool,
}

/// One creature that may attack and the defenders it may attack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AttackerOption {
    /// The creature.
    pub object: ObjectId,
    /// Players and planeswalkers it may attack.
    pub defenders: Vec<AttackTarget>,
}

/// Legal attackers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AttackOptions {
    /// Every creature that may attack; choosing none is always legal.
    pub attackers: Vec<AttackerOption>,
}

/// One creature that may block and the attackers it may block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BlockerOption {
    /// The potential blocker.
    pub object: ObjectId,
    /// Attackers it may block.
    pub can_block: Vec<ObjectId>,
}

/// Legal blockers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BlockOptions {
    /// Every creature that may block; choosing none is always legal.
    pub blockers: Vec<BlockerOption>,
}

/// An attacker that must divide its damage among blockers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DamageOptions {
    /// The attacking creature.
    pub attacker: ObjectId,
    /// Damage to divide; all of it must be assigned.
    pub power: u32,
    /// The blockers that may receive damage.
    pub blockers: Vec<ObjectId>,
}

/// Options for the cleanup discard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DiscardOptions {
    /// Cards in hand to choose from.
    pub hand: Vec<ObjectId>,
    /// Exactly this many must be chosen.
    pub count: usize,
}

/// Keep or mulligan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum MulliganChoice {
    /// Keep this hand.
    Keep,
    /// Shuffle it away and draw a new one.
    Mulligan,
}

/// What a player does with priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum PriorityAction {
    /// Pass priority.
    Pass,
    /// Play this land from hand.
    PlayLand(ObjectId),
    /// Start casting this spell from hand (leads to a payment decision).
    CastSpell(ObjectId),
    /// Activate this mana ability.
    ActivateMana(ManaSource),
}

/// How to pay for a spell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum PayChoice {
    /// Let the engine pick a valid payment.
    Auto,
    /// Pay exactly this.
    Manual(Payment),
    /// Pay these parts and let the engine pay the rest of the cost.
    Complete(Payment),
    /// Abandon the cast (when `can_cancel`): the spell stays where it was, nothing is logged.
    Cancel,
}

/// A player's answer to the pending decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Answer {
    /// Keep or mulligan.
    Mulligan(MulliganChoice),
    /// Cards to bottom.
    BottomCards(Vec<ObjectId>),
    /// A priority action.
    Priority(PriorityAction),
    /// A payment choice.
    PayMana(PayChoice),
    /// Attackers and what each attacks; empty means no attack.
    DeclareAttackers(Vec<(ObjectId, AttackTarget)>),
    /// Pairs of (blocker, attacker blocked); empty means no blocks.
    DeclareBlockers(Vec<(ObjectId, ObjectId)>),
    /// Damage per blocker.
    AssignDamage(Vec<(ObjectId, u32)>),
    /// Cards to discard.
    Discard(Vec<ObjectId>),
}

impl Answer {
    /// Short name used in error messages, matching [`DecisionKind::name`].
    pub const fn name(&self) -> &'static str {
        match self {
            Answer::Mulligan(_) => "mulligan",
            Answer::BottomCards(_) => "bottom-cards",
            Answer::Priority(_) => "priority",
            Answer::PayMana(_) => "pay-mana",
            Answer::DeclareAttackers(_) => "declare-attackers",
            Answer::DeclareBlockers(_) => "declare-blockers",
            Answer::AssignDamage(_) => "assign-damage",
            Answer::Discard(_) => "discard",
        }
    }
}
