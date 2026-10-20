//! Decisions the engine asks for and the answers it accepts.
//!
//! A [`Decision`] lists every legal option; an [`Answer`] must be built from exactly those.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use serde::{Deserialize, Serialize};

use crate::abilities::AbilityRef;
use crate::choose::{
    CardsChoice, CardsOptions, CostsChoice, CostsOptions, DivideChoice, DivideOptions, ModeChoice,
    ModeOptions, NumberChoice, NumberOptions, OptionChoice, OptionOptions, OrderChoice,
    OrderOptions, ReplacementChoice, ReplacementOptions, TargetChoice, TargetOptions,
    TriggersChoice, TriggersOptions, YesNoChoice, YesNoOptions,
};
use crate::combat::{AttackTarget, DamageTarget};
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
    /// Choose the targets of one target specification as a spell or ability goes on the stack.
    ChooseTargets(TargetOptions),
    /// Choose the modes of a modal spell or ability.
    ChooseMode(ModeOptions),
    /// Choose a number (X, or "any number").
    ChooseNumber(NumberOptions),
    /// Divide damage or counters among the targets.
    DivideAmount(DivideOptions),
    /// Choose cards from a list; any seat may be asked.
    ChooseCards(CardsOptions),
    /// Put cards in an order; any seat may be asked.
    OrderCards(OrderOptions),
    /// Answer a "may" or a "pay or not"; any seat may be asked.
    YesNo(YesNoOptions),
    /// Choose one of a short list (a colour, a player, a name); any seat may be asked.
    ChooseOption(OptionOptions),
    /// Choose which optional costs to pay.
    ChooseCosts(CostsOptions),
    /// Order the triggered abilities that go on the stack together.
    OrderTriggers(TriggersOptions),
    /// Choose which replacement effect applies first.
    ChooseReplacement(ReplacementOptions),
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
            DecisionKind::ChooseTargets(_) => "choose-targets",
            DecisionKind::ChooseMode(_) => "choose-mode",
            DecisionKind::ChooseNumber(_) => "choose-number",
            DecisionKind::DivideAmount(_) => "divide-amount",
            DecisionKind::ChooseCards(_) => "choose-cards",
            DecisionKind::OrderCards(_) => "order-cards",
            DecisionKind::YesNo(_) => "yes-no",
            DecisionKind::ChooseOption(_) => "choose-option",
            DecisionKind::ChooseCosts(_) => "choose-costs",
            DecisionKind::OrderTriggers(_) => "order-triggers",
            DecisionKind::ChooseReplacement(_) => "choose-replacement",
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
    /// Abilities that are not mana abilities and can be activated now; a player who has one has something to do.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<AbilityRef>>", optional))]
    pub abilities: Vec<AbilityRef>,
    /// Whether anything useful can be done now: a land to play, a spell to cast or an ability that is not a
    /// mana ability. Tapping for mana alone never counts, so a player with only `mana_abilities` has nothing to do.
    pub can_act: bool,
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
    /// Whether an automatic payment exists (it does whenever the cast was offered, unless only a payment that takes the last life would do).
    pub auto_available: bool,
    /// Whether the cast may be backed out of; nothing has been paid yet, so it rolls back (CR 601.2).
    pub can_cancel: bool,
    /// The payer's life total.
    pub life: i32,
    /// The most life this payment may take: 2 for each Phyrexian symbol in the cost, within the life total (CR 119.4).
    pub max_life: u32,
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
    /// Every creature that may attack; choosing none is legal unless a rule requires an attacker.
    pub attackers: Vec<AttackerOption>,
    /// The creatures among `attackers` that a rule requires to attack (CR 508.1d); the declaration names each of them.
    /// Left out while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub required: Vec<ObjectId>,
}

impl AttackOptions {
    /// The declaration that does only what the rules require: each creature that must attack, at the first defender
    /// it may attack.
    pub fn required_attack(&self) -> Vec<(ObjectId, AttackTarget)> {
        self.attackers
            .iter()
            .filter(|option| self.required.contains(&option.object))
            .filter_map(|option| {
                option
                    .defenders
                    .first()
                    .map(|first| (option.object, *first))
            })
            .collect()
    }
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
    /// The fewest creatures that may block an attacker together, for each attacker that needs more than one
    /// (menace): it is blocked by that many or by none. Left out while no attacker needs several.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<(ObjectId, usize)>>", optional))]
    pub min_blockers: Vec<(ObjectId, usize)>,
    /// The creatures among `blockers` that a rule requires to block (CR 509.1c); the declaration names each of them,
    /// together with the creatures an attacker that needs several blockers must have. Left out while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub required: Vec<ObjectId>,
}

impl BlockOptions {
    /// How many creatures must block `attacker` together if any do.
    pub fn needed_for(&self, attacker: ObjectId) -> usize {
        self.min_blockers
            .iter()
            .find(|(menaced, _)| *menaced == attacker)
            .map_or(1, |(_, needed)| *needed)
    }

    /// The declaration that does only what the rules require: each creature that must block, at an attacker that
    /// is blocked already or needs one blocker if there is one, and free creatures to join an attacker that needs
    /// several. It is legal unless the free creatures run out.
    pub fn required_blocks(&self) -> Vec<(ObjectId, ObjectId)> {
        let mut blocks: Vec<(ObjectId, ObjectId)> = Vec::new();
        for option in self
            .blockers
            .iter()
            .filter(|option| self.required.contains(&option.object))
        {
            let easy = option.can_block.iter().copied().find(|attacker| {
                self.needed_for(*attacker) <= 1 || blocks.iter().any(|(_, held)| held == attacker)
            });
            if let Some(attacker) = easy.or_else(|| option.can_block.first().copied()) {
                blocks.push((option.object, attacker));
            }
        }
        let mut attackers: Vec<ObjectId> = Vec::new();
        for (_, attacker) in &blocks {
            if !attackers.contains(attacker) {
                attackers.push(*attacker);
            }
        }
        for attacker in attackers {
            while blocks.iter().filter(|(_, held)| *held == attacker).count()
                < self.needed_for(attacker)
            {
                let free = self.blockers.iter().find(|option| {
                    option.can_block.contains(&attacker)
                        && !blocks.iter().any(|(blocker, _)| *blocker == option.object)
                });
                match free {
                    Some(option) => blocks.push((option.object, attacker)),
                    None => break,
                }
            }
        }
        blocks
    }

    /// Whether `blocker`, which the declaration `blocks` leaves out, could be added to it without breaking a rule: it
    /// could block an attacker that is blocked already or needs one blocker, or one that needs several and has
    /// creatures free to join it.
    pub fn could_join(&self, blocker: ObjectId, blocks: &[(ObjectId, ObjectId)]) -> bool {
        let Some(option) = self.blockers.iter().find(|option| option.object == blocker) else {
            return false;
        };
        option.can_block.iter().any(|attacker| {
            let held = blocks.iter().filter(|(_, at)| at == attacker).count();
            let needed = self.needed_for(*attacker);
            if held > 0 || needed <= 1 {
                return true;
            }
            let free = self
                .blockers
                .iter()
                .filter(|other| {
                    other.object != blocker
                        && other.can_block.contains(attacker)
                        && !blocks.iter().any(|(held, _)| *held == other.object)
                })
                .count();
            free + 1 >= needed
        })
    }
}

/// An attacker that must divide its damage among blockers and, with trample, the player or planeswalker it attacks.
///
/// The answer lists the share of each blocker. Without `excess` the shares add up to `power`. With it they
/// add up to at most `power` and what is left goes to `excess`, which takes damage only when every blocker has
/// at least its `lethal` damage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DamageOptions {
    /// The attacking creature.
    pub attacker: ObjectId,
    /// Damage to divide.
    pub power: u32,
    /// The blockers that may receive damage.
    pub blockers: Vec<ObjectId>,
    /// With trample, the player or planeswalker the attacker attacks, which takes the damage not given to a blocker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub excess: Option<DamageTarget>,
    /// Damage that is lethal to each blocker, in the order of `blockers`: what its toughness leaves after the
    /// damage marked, and at most 1 from a source with deathtouch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<u32>>", optional))]
    pub lethal: Vec<u32>,
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
    /// Activate this ability, which is not a mana ability (leads to its announcement and costs).
    Activate(AbilityRef),
}

/// How to pay for a spell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum PayChoice {
    /// Let the engine pick a valid payment: mana if it can, life only when it must.
    Auto,
    /// Pay exactly this, life included.
    Manual(Payment),
    /// Pay these parts (mana and life) and let the engine pay the rest of the cost.
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
    /// The targets of the specification asked about, or backing out of the cast.
    ChooseTargets(TargetChoice),
    /// The modes chosen.
    ChooseMode(ModeChoice),
    /// The number chosen.
    ChooseNumber(NumberChoice),
    /// How the amount is divided.
    DivideAmount(DivideChoice),
    /// The cards chosen.
    ChooseCards(CardsChoice),
    /// The cards in the order chosen.
    OrderCards(OrderChoice),
    /// Yes or no.
    YesNo(YesNoChoice),
    /// The option chosen.
    ChooseOption(OptionChoice),
    /// The optional costs chosen.
    ChooseCosts(CostsChoice),
    /// The order of the triggers.
    OrderTriggers(TriggersChoice),
    /// The replacement effect applied first.
    ChooseReplacement(ReplacementChoice),
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
            Answer::ChooseTargets(_) => "choose-targets",
            Answer::ChooseMode(_) => "choose-mode",
            Answer::ChooseNumber(_) => "choose-number",
            Answer::DivideAmount(_) => "divide-amount",
            Answer::ChooseCards(_) => "choose-cards",
            Answer::OrderCards(_) => "order-cards",
            Answer::YesNo(_) => "yes-no",
            Answer::ChooseOption(_) => "choose-option",
            Answer::ChooseCosts(_) => "choose-costs",
            Answer::OrderTriggers(_) => "order-triggers",
            Answer::ChooseReplacement(_) => "choose-replacement",
        }
    }
}
