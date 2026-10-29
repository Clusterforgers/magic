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
use crate::help::{Help, Offered};
use crate::mana::{ManaOption, ManaPool, ManaSource, Payment};
use crate::manual::ManualAction;
use crate::spend::Spend;

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
    Priority(Box<PriorityOptions>),
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
    /// Cards in the hand one more mulligan draws, as the table's mulligan style sizes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub hand_if_mulligan: Option<usize>,
    /// Cards scried after keeping this hand; left out when none are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub scry_if_keep: Option<usize>,
    /// Cards scried after keeping the hand one more mulligan draws; left out when none are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub scry_if_mulligan: Option<usize>,
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
    /// Cards in the hand that can be suspended now (a special action that does not use the stack, CR 702.62).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub suspendable: Vec<ObjectId>,
    /// Cards in the hand that can be foretold now (a special action during their owner's turn that does not use the stack, CR 702.143).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub foretellable: Vec<ObjectId>,
    /// Face-down permanents their controller can turn face up now (a special action that does not use the stack, CR 702.37e).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub turnable: Vec<ObjectId>,
    /// Whether the holder can put their companion into their hand now: a special action (CR 702.139) for {3} at sorcery
    /// timing, once a game.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub companion: bool,
    /// The faces of the cards with several faces that can be cast or played now, with their costs: a split card's halves, an
    /// Adventure, the back face of a modal double-faced card. `castable` and `playable_lands` list the card itself. A
    /// prepared permanent whose spell its controller can cast is listed with its spell face and no entry in `castable`
    /// (`PriorityAction::CastPrepared`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<FaceOption>>", optional))]
    pub faces: Vec<FaceOption>,
    /// Whether anything useful can be done now: a land to play, a spell to cast or an ability that is not a
    /// mana ability. Tapping for mana alone never counts, so a player with only `mana_abilities` has nothing to do.
    pub can_act: bool,
    /// Passing is always legal.
    pub can_pass: bool,
}

/// A face of a card with several faces that a player can cast or play now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FaceOption {
    /// The card.
    pub card: ObjectId,
    /// The face, as its position among the card's faces.
    pub face: u8,
    /// The name of the face.
    pub name: String,
    /// What it costs to cast; none for a land.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cost: Option<ManaCost>,
    /// Whether the face is a land to play, not a spell to cast.
    pub land: bool,
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
    /// The creatures that may be tapped (`convoke`) and the graveyard cards that may be exiled (`delve`) to pay part of the cost; boxed to keep a decision small, and flat on the wire.
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub help: Box<Offered>,
    /// What the payment is for, which decides what restricted mana may be spent on it; left out means neither a spell nor an ability.
    #[serde(default, skip_serializing_if = "Spend::is_other")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Spend>", optional))]
    pub spend: Box<Spend>,
    /// Whether an automatic payment exists (it does whenever the cast was offered, unless only a payment that takes the last life would do).
    pub auto_available: bool,
    /// Whether the cast may be backed out of; nothing has been paid yet, so it rolls back (CR 601.2).
    pub can_cancel: bool,
    /// The payer's life total.
    pub life: i32,
    /// The most life this payment may take: 2 for each Phyrexian symbol in the cost, within the life total (CR 119.4).
    pub max_life: u32,
}

impl PayOptions {
    /// The creatures and cards that may help pay, as the payment functions read them.
    pub fn help(&self) -> Help<'_> {
        self.help.help()
    }
}

/// Whether a count is zero, so serialization can leave it out.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(count: &u32) -> bool {
    *count == 0
}

/// Whether a flag is off, so serialization can leave it out.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

/// One creature that may attack and the defenders it may attack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AttackerOption {
    /// The creature.
    pub object: ObjectId,
    /// Players and planeswalkers it may attack.
    pub defenders: Vec<AttackTarget>,
    /// What it costs to attack each defender that an effect taxes (CR 508.1h): the attack is declared with the payment.
    /// Left out while it costs nothing to attack any of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<Vec<(AttackTarget, ManaCost)>>", optional)
    )]
    pub tax: Vec<(AttackTarget, ManaCost)>,
    /// The life it costs to attack each defender that an effect taxes in life ("pays 2 life for each of those
    /// creatures"), paid as the attack is declared. Left out while no defender costs life.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<(AttackTarget, u32)>>", optional))]
    pub life: Vec<(AttackTarget, u32)>,
}

impl AttackerOption {
    /// An attacker that costs nothing to attack with.
    pub fn free(object: ObjectId, defenders: Vec<AttackTarget>) -> Self {
        AttackerOption {
            object,
            defenders,
            tax: Vec::new(),
            life: Vec::new(),
        }
    }

    /// The first defender it may attack for nothing.
    pub fn free_defender(&self) -> Option<AttackTarget> {
        self.defenders.iter().copied().find(|target| {
            !self.tax.iter().any(|(taxed, _)| taxed == target)
                && !self.life.iter().any(|(taxed, _)| taxed == target)
        })
    }
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
    /// The mana the player can surely spend on what the attack costs, whichever creatures attack (a floor, not a limit):
    /// the mana floating and that of the untapped permanents that are not creatures that may attack. Left out while
    /// nothing is taxed.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
    pub mana: u32,
    /// The most creatures that may attack together (CR 508.1c: "no more than one creature can attack each combat"). Left
    /// out while no rule limits the declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub most: Option<usize>,
    /// The most creatures that may attack each limited player or planeswalker together (CR 508.1c: "no more than two
    /// creatures can attack you each combat"). Left out while no rule limits the attacks at one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<Vec<(AttackTarget, usize)>>", optional)
    )]
    pub most_at: Vec<(AttackTarget, usize)>,
}

impl AttackOptions {
    /// The declaration that does only what the rules require: each creature that must attack, at the first defender
    /// it may attack for nothing that has room; no more of them than the limits let attack.
    pub fn required_attack(&self) -> Vec<(ObjectId, AttackTarget)> {
        let first: Vec<(ObjectId, AttackTarget)> = self
            .attackers
            .iter()
            .filter(|option| self.required.contains(&option.object))
            .filter_map(|option| option.free_defender().map(|first| (option.object, first)))
            .collect();
        self.w8a_capped(&first)
    }

    /// What the declaration costs, each attacker for the defender it attacks (CR 508.1h).
    pub fn cost_of(&self, attackers: &[(ObjectId, AttackTarget)]) -> ManaCost {
        let symbols = attackers
            .iter()
            .filter_map(|(object, target)| {
                let option = self
                    .attackers
                    .iter()
                    .find(|option| option.object == *object)?;
                option
                    .tax
                    .iter()
                    .find(|(taxed, _)| taxed == target)
                    .map(|(_, cost)| cost.symbols().to_vec())
            })
            .flatten();
        ManaCost::from_symbols(symbols)
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
    /// How many of them it may block beyond the first ("can block an additional creature each combat"): the
    /// declaration names it once for each. Left out while it is 0.
    #[serde(default, skip_serializing_if = "w14c_is_zero")]
    #[cfg_attr(feature = "ts", ts(as = "Option<usize>", optional))]
    pub extra: usize,
}

/// Whether a count is 0, so it is left out of the wire form.
// serde's `skip_serializing_if` hands out a reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn w14c_is_zero(count: &usize) -> bool {
    *count == 0
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
    /// The most creatures that may block an attacker together, for each attacker that is limited ("can't be blocked
    /// by more than one creature"). Left out while no attacker is.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<(ObjectId, usize)>>", optional))]
    pub max_blockers: Vec<(ObjectId, usize)>,
    /// The creatures among `blockers` that a rule requires to block (CR 509.1c); the declaration names each of them,
    /// together with the creatures an attacker that needs several blockers must have. Left out while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub required: Vec<ObjectId>,
    /// The most creatures that may block together (CR 509.1b: "no more than one creature can block each combat"). Left
    /// out while no rule limits the declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub most: Option<usize>,
}

impl BlockOptions {
    /// How many creatures must block `attacker` together if any do.
    pub fn needed_for(&self, attacker: ObjectId) -> usize {
        self.min_blockers
            .iter()
            .find(|(menaced, _)| *menaced == attacker)
            .map_or(1, |(_, needed)| *needed)
    }

    /// How many creatures may block `attacker` together at most.
    pub fn limit_for(&self, attacker: ObjectId) -> usize {
        self.max_blockers
            .iter()
            .find(|(limited, _)| *limited == attacker)
            .map_or(usize::MAX, |(_, most)| *most)
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
            let room = |attacker: &ObjectId| {
                blocks.iter().filter(|(_, held)| held == attacker).count()
                    < self.limit_for(*attacker)
            };
            let easy = option.can_block.iter().copied().find(|attacker| {
                room(attacker)
                    && (self.needed_for(*attacker) <= 1
                        || blocks.iter().any(|(_, held)| held == attacker))
            });
            let any = || option.can_block.iter().copied().find(room);
            if let Some(attacker) = easy.or_else(any) {
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
        self.w8a_capped(&blocks)
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
            if held >= self.limit_for(*attacker) {
                return false;
            }
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum PriorityAction {
    /// Pass priority.
    Pass,
    /// Play this land from hand.
    PlayLand(ObjectId),
    /// Play this land from hand as the face of a card with several faces at this position (CR 712.12).
    PlayLandFace(ObjectId, u8),
    /// Start casting this spell from hand (leads to a payment decision).
    CastSpell(ObjectId),
    /// Activate this mana ability.
    ActivateMana(ManaSource),
    /// Activate this ability, which is not a mana ability (leads to its announcement and costs).
    Activate(AbilityRef),
    /// Suspend this card from the hand (CR 702.62, a special action): pay its suspend cost and exile it with time counters.
    Suspend(ObjectId),
    /// Foretell this card from the hand (CR 702.143, a special action): pay {2} and exile it face down.
    Foretell(ObjectId),
    /// Turn this face-down permanent face up (CR 702.37e, a special action): pay its morph, megamorph or disguise cost.
    TurnFaceUp(ObjectId),
    /// Cast a copy of the spell face of this prepared permanent (it must be offered in `PriorityOptions.faces`): the copy is made and the
    /// cast is announced and paid like any spell, at the timing of the face; the permanent is unprepared as the spell is cast.
    CastPrepared(ObjectId),
    /// Put the companion into the hand (CR 702.139, a special action): pay {3}, once a game.
    BringCompanion,
    /// Do something by hand for a card the engine does not run: a zone move, counters, life, a token and the like. Legal at
    /// any priority, checked against the state (`manual.rs`).
    Manual(ManualAction),
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

#[cfg(test)]
mod tests {
    //! Goal: the declaration that does only what the rules require attacks with each creature that must, at the first defender it may attack, and blocks with each creature that must, at an attacker that is blocked already or needs one blocker if there is one and with free creatures to join an attacker that needs several; a creature left out of a declaration could join it exactly when some attacker it can block is blocked already, needs one blocker or has enough free creatures to join.
    use rstest::rstest;

    use super::*;
    use crate::combat::AttackTarget;

    fn id(n: u64) -> ObjectId {
        ObjectId(n)
    }

    /// Blockers as (creature, attackers it can block), the attackers that need several as (attacker, fewest), and the
    /// creatures a rule requires.
    fn blocking(
        blockers: &[(u64, &[u64])],
        menace: &[(u64, usize)],
        required: &[u64],
    ) -> BlockOptions {
        BlockOptions {
            most: None,
            blockers: blockers
                .iter()
                .map(|(object, can_block)| BlockerOption {
                    object: id(*object),
                    can_block: can_block.iter().map(|attacker| id(*attacker)).collect(),
                    extra: 0,
                })
                .collect(),
            min_blockers: menace.iter().map(|(a, n)| (id(*a), *n)).collect(),
            max_blockers: Vec::new(),
            required: required.iter().map(|object| id(*object)).collect(),
        }
    }

    fn pairs(blocks: &[(u64, u64)]) -> Vec<(ObjectId, ObjectId)> {
        blocks.iter().map(|(b, a)| (id(*b), id(*a))).collect()
    }

    #[rstest]
    #[case::nothing_is_required(&[(1, &[10][..])], &[], &[], &[])]
    #[case::one_blocker_one_attacker(&[(1, &[10][..])], &[], &[1], &[(1, 10)])]
    #[case::the_first_attacker_that_needs_one(&[(1, &[10, 11][..])], &[(10, 2)], &[1], &[(1, 11)])]
    #[case::an_attacker_blocked_already(&[(1, &[10][..]), (2, &[10, 11][..])], &[(10, 2)], &[1, 2], &[(1, 10), (2, 10)])]
    #[case::a_free_creature_joins_a_menace_attacker(&[(1, &[10][..]), (2, &[10][..])], &[(10, 2)], &[1], &[(1, 10), (2, 10)])]
    #[case::two_required_meet_on_one(&[(1, &[10][..]), (2, &[10][..])], &[(10, 2)], &[1, 2], &[(1, 10), (2, 10)])]
    #[case::too_few_free_creatures(&[(1, &[10][..])], &[(10, 2)], &[1], &[(1, 10)])]
    fn the_required_blocks_are_the_least_the_rules_ask(
        #[case] blockers: &[(u64, &[u64])],
        #[case] menace: &[(u64, usize)],
        #[case] required: &[u64],
        #[case] expected: &[(u64, u64)],
    ) {
        let options = blocking(blockers, menace, required);
        assert_eq!(options.required_blocks(), pairs(expected));
    }

    #[rstest]
    #[case::an_attacker_that_needs_one(&[(1, &[10][..])], &[], &[], true)]
    #[case::an_attacker_blocked_already(&[(1, &[10][..]), (2, &[10][..])], &[(10, 2)], &[(2, 10), (3, 10)], true)]
    #[case::a_menace_attacker_with_a_free_creature(&[(1, &[10][..]), (2, &[10][..])], &[(10, 2)], &[], true)]
    #[case::a_menace_attacker_with_no_free_creature(&[(1, &[10][..]), (2, &[10][..])], &[(10, 2)], &[(2, 11)], false)]
    #[case::a_menace_attacker_alone(&[(1, &[10][..])], &[(10, 2)], &[], false)]
    fn a_creature_left_out_could_join_when_an_attacker_it_can_block_can_take_it(
        #[case] blockers: &[(u64, &[u64])],
        #[case] menace: &[(u64, usize)],
        #[case] blocks: &[(u64, u64)],
        #[case] could: bool,
    ) {
        let options = blocking(blockers, menace, &[1]);
        assert_eq!(options.could_join(id(1), &pairs(blocks)), could);
    }

    /// `blocking` with attackers that may be blocked by so many creatures at most.
    fn limited(blockers: &[(u64, &[u64])], max: &[(u64, usize)], required: &[u64]) -> BlockOptions {
        BlockOptions {
            most: None,
            max_blockers: max.iter().map(|(a, n)| (id(*a), *n)).collect(),
            ..blocking(blockers, &[], required)
        }
    }

    #[rstest]
    #[case::an_attacker_with_no_limit(&[], 10, usize::MAX)]
    #[case::one_at_most(&[(10, 1)], 10, 1)]
    #[case::another_attacker_is_not_limited(&[(10, 1)], 11, usize::MAX)]
    fn the_limit_of_an_attacker_is_its_entry_or_none(
        #[case] max: &[(u64, usize)],
        #[case] attacker: u64,
        #[case] limit: usize,
    ) {
        let options = limited(&[(1, &[10, 11][..])], max, &[]);
        assert_eq!(options.limit_for(id(attacker)), limit);
    }

    #[rstest]
    #[case::the_attacker_is_blocked_already(&[(2, 10)], false)]
    #[case::the_attacker_is_not_blocked(&[], true)]
    #[case::the_other_attacker_is_blocked(&[(2, 11)], true)]
    fn a_creature_cannot_join_an_attacker_that_has_its_blockers(
        #[case] blocks: &[(u64, u64)],
        #[case] could: bool,
    ) {
        let options = limited(&[(1, &[10][..]), (2, &[10, 11][..])], &[(10, 1)], &[]);
        assert_eq!(options.could_join(id(1), &pairs(blocks)), could);
    }

    #[rstest]
    #[case::the_second_creature_goes_elsewhere(&[(1, &[10][..]), (2, &[10, 11][..])], &[(10, 1)], &[1, 2], &[(1, 10), (2, 11)])]
    #[case::the_second_creature_has_no_place(&[(1, &[10][..]), (2, &[10][..])], &[(10, 1)], &[1, 2], &[(1, 10)])]
    #[case::an_attacker_with_room(&[(1, &[10][..]), (2, &[10][..])], &[(10, 2)], &[1, 2], &[(1, 10), (2, 10)])]
    fn the_required_blocks_keep_to_the_limits(
        #[case] blockers: &[(u64, &[u64])],
        #[case] max: &[(u64, usize)],
        #[case] required: &[u64],
        #[case] expected: &[(u64, u64)],
    ) {
        let options = limited(blockers, max, required);
        assert_eq!(options.required_blocks(), pairs(expected));
    }

    #[test]
    fn a_creature_that_is_not_offered_cannot_join() {
        let options = blocking(&[(2, &[10][..])], &[], &[]);
        assert!(!options.could_join(id(1), &[]));
    }

    #[test]
    fn the_required_attack_names_each_required_creature_at_its_first_defender() {
        let attack = |object: u64, defenders: &[u64]| {
            AttackerOption::free(
                id(object),
                defenders
                    .iter()
                    .map(|player| AttackTarget::Player(PlayerId(*player)))
                    .collect(),
            )
        };
        let options = AttackOptions {
            most: None,
            most_at: Vec::new(),
            attackers: vec![attack(1, &[2, 3]), attack(2, &[3]), attack(3, &[])],
            required: vec![id(1), id(3)],
            mana: 0,
        };
        assert_eq!(
            options.required_attack(),
            [(id(1), AttackTarget::Player(PlayerId(2)))],
            "a creature with no defender cannot be declared"
        );
    }

    #[test]
    fn the_new_fields_are_left_out_of_the_json_while_empty() {
        let plain = AttackOptions {
            most: None,
            most_at: Vec::new(),
            attackers: Vec::new(),
            required: Vec::new(),
            mana: 0,
        };
        assert_eq!(
            serde_json::to_string(&plain).unwrap(),
            r#"{"attackers":[]}"#
        );
        let blocks = blocking(&[], &[], &[]);
        assert_eq!(
            serde_json::to_string(&blocks).unwrap(),
            r#"{"blockers":[]}"#
        );
        let required = AttackOptions {
            most: None,
            required: vec![id(4)],
            ..plain
        };
        assert_eq!(
            serde_json::to_string(&required).unwrap(),
            r#"{"attackers":[],"required":[4]}"#
        );
    }
}
