//! Pace of play: passing for a seat the windows pass for or that yields, and the clocks that keep one player from
//! holding the table.
//!
//! All of them work through the same path as a seat's own answer ([`crate::running::Running::answer`]), so what a
//! pass or a timeout does to the log, the store and every projection is exactly what the seat's click would have done.
//! When a seat is asked is decided in [`crate::windows`] and when a yield stops in [`crate::yields`], alone. A seat that
//! cannot act at a window where it waits is passed for the same way once its dwell ([`crate::dwell`]) is over.

use std::time::Duration;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{
    CardsChoice, CardsOptions, CostsChoice, DivideChoice, DivideOptions, ModeChoice, ModeOptions,
    NumberChoice, OptionChoice, OrderChoice, ReplacementChoice, Target, TargetChoice,
    TriggersChoice, YesNoChoice,
};
use mtg_engine::decision::{DamageOptions, MulliganChoice, PayChoice, PayOptions, PriorityAction};
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{ErrorCode, FullControlMode, ServerMessage, YieldUntil};
use tokio::time::Instant;

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::dwell::Dwell;
use crate::running::{Extended, Halt, Rejection};
use crate::store::EventStore;
use crate::windows::Verdict;
use crate::yields::Yields;

/// Longest clock the settings accept, in seconds (a day), so a deadline can never overflow.
pub const MAX_CLOCK_SECS: u64 = 86_400;

/// Own turns in a row, none of them with a clock running out, that earn a seat one extension.
pub const BANK_TURNS: u8 = 3;

/// Most extensions a seat holds; one earned past it is lost.
pub const BANK_MAX: u8 = 3;

/// How long after a failed save the server tries again to answer for a seat whose clock ran out.
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// How a game moves: how seats start out (auto-pass on, or in Full Control), how long a seat may take to respond and how
/// long the table waits before it passes for a seat that cannot act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pace {
    /// Whether each seat starts with auto-pass on (`MTG_AUTO_PASS`): in Full Control mode `Off`, or `Held` when off.
    /// A seat can change its own with `AutoPass` and `FullControl`.
    pub auto_pass: bool,
    /// Time a seat that is not the active player has to answer a priority decision, whether it can act or not, and to
    /// choose the targets of a spell or ability, order its triggers, answer a yes-or-no question, choose an option,
    /// choose modes, choose cards or put cards in order (`MTG_RESPONSE_SECS`); `None` is no clock.
    pub response: Option<Duration>,
    /// Time a defending seat has to declare blockers (`MTG_BLOCK_SECS`); `None` is no clock.
    pub blocks: Option<Duration>,
    /// Time to decide what takes thought and is no quick response (`MTG_THINK_SECS`): to keep or mulligan and to bottom
    /// cards (every seat: the opening has no active player), and to pay for a spell, divide combat damage, declare
    /// attackers or discard (a seat that is not the active player); `None` is no clock.
    pub think: Option<Duration>,
    /// Time the active player has for every decision of its own turn after the opening (`MTG_ACTIVE_SECS`); `None` is
    /// no clock.
    pub active: Option<Duration>,
    /// Time an extension adds to a clock that runs out (`MTG_EXTENSION_SECS`); `None` turns the bank off.
    pub extension: Option<Duration>,
    /// How long a clock runs before its countdown shows (`MTG_CLOCK_GRACE_SECS`); a clock no longer than this shows at
    /// once, as does zero.
    pub grace: Duration,
    /// How long the table waits before it passes for an idle seat at a window of the default table, its own main phase
    /// with an empty stack aside (`MTG_DWELL_MS`).
    pub dwell: Dwell,
}

/// What kind of clock a decision runs on, before the seat is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// Asked before any turn exists: every seat is hurried the same.
    Opening,
    /// What a seat answers at the table.
    Response,
    /// Declaring blockers.
    Blocks,
    /// What takes thought.
    Think,
}

impl Class {
    /// The class of `kind`. Every decision kind is named here, so a new one cannot be added without choosing its clock.
    pub(crate) fn of(kind: &DecisionKind) -> Class {
        match kind {
            DecisionKind::Mulligan(_) | DecisionKind::BottomCards(_) => Class::Opening,
            // A target, an order or a question is asked of a seat the table waits for just as a priority window is.
            DecisionKind::Priority(_)
            | DecisionKind::ChooseTargets(_)
            | DecisionKind::OrderTriggers(_)
            | DecisionKind::ChooseReplacement(_)
            | DecisionKind::YesNo(_)
            | DecisionKind::ChooseOption(_)
            | DecisionKind::ChooseMode(_)
            | DecisionKind::ChooseNumber(_)
            | DecisionKind::DivideAmount(_)
            | DecisionKind::ChooseCosts(_)
            | DecisionKind::ChooseCards(_)
            | DecisionKind::OrderCards(_) => Class::Response,
            DecisionKind::DeclareBlockers(_) => Class::Blocks,
            DecisionKind::PayMana(_)
            | DecisionKind::AssignDamage(_)
            | DecisionKind::DeclareAttackers(_)
            | DecisionKind::Discard(_) => Class::Think,
        }
    }
}

impl Pace {
    /// No auto-pass and no clocks: every decision waits for its player, as in a game played straight on an engine.
    pub const MANUAL: Pace = Pace {
        auto_pass: false,
        response: None,
        blocks: None,
        think: None,
        active: None,
        extension: None,
        grace: Duration::ZERO,
        dwell: Dwell::OFF,
    };

    /// The Full Control mode every seat starts in: `Off` with auto-pass on, `Held` with it off.
    pub(crate) const fn mode(&self) -> FullControlMode {
        if self.auto_pass {
            FullControlMode::Off
        } else {
            FullControlMode::Held
        }
    }

    /// The clock of every decision of the opening, whatever its kind and seat: keep or mulligan, bottoming and the scry
    /// of a seat that kept after a mulligan.
    pub(crate) fn opening_clock(&self) -> Option<Duration> {
        self.think
    }

    /// The clock `decision` runs on, if any.
    ///
    /// The opening decisions (keep or mulligan, bottoming) are asked before any turn exists and run on `think` for
    /// every seat. After that the active player's decisions run on `active`, whatever their kind, and every other seat
    /// is hurried by the class of the kind ([`Class::of`]): a priority decision, the targets of a spell or ability, the
    /// order of its triggers or of the replacement effects that apply to an event and the questions of effects (yes or
    /// no, an option, modes, a number, optional costs, a division, cards, an order of cards) run on `response`,
    /// blockers on `blocks`, and paying for a spell, dividing combat damage, declaring attackers and discarding on
    /// `think`. A priority window the seat has nothing to do in is hurried too (it is only delivered at a stop or in
    /// Full Control): it would hold the table for no reason. The rule reads the kind and the seat alone, never the
    /// options or the hand, so it shows nothing about them.
    pub(crate) fn clock_for(&self, decision: &Decision, active: PlayerId) -> Option<Duration> {
        match Class::of(&decision.kind) {
            Class::Opening => self.think,
            _ if decision.player == active => self.active,
            Class::Response => self.response,
            Class::Blocks => self.blocks,
            Class::Think => self.think,
        }
    }

    /// How long after it is issued a clock of `length` shows its countdown: the grace, or at once when the clock is no
    /// longer than that. It reads the length alone, so it follows the same rule as the clock.
    pub(crate) fn shown_after(&self, length: Duration) -> Duration {
        if self.grace < length {
            self.grace
        } else {
            Duration::ZERO
        }
    }
}

impl Default for Pace {
    fn default() -> Self {
        Pace {
            auto_pass: true,
            response: Some(Duration::from_secs(10)),
            blocks: Some(Duration::from_secs(45)),
            think: Some(Duration::from_secs(60)),
            active: None,
            extension: Some(Duration::from_secs(20)),
            grace: Duration::from_secs(3),
            dwell: Dwell::DEFAULT,
        }
    }
}

/// A priority pass.
pub(crate) fn pass() -> Answer {
    Answer::Priority(PriorityAction::Pass)
}

/// Whether the decision is a priority decision, whatever its options.
pub(crate) fn is_priority(decision: &Decision) -> bool {
    matches!(&decision.kind, DecisionKind::Priority(_))
}

/// What the table answers for a seat whose clock ran out: a pass, only the blockers or attackers a rule requires, no
/// target (a spell is taken back and a triggered ability removed, as the rules do when a target cannot be chosen), the
/// triggers in the order they triggered, No to a question, the first option, for modes the cast taken back when the
/// options allow it and the first mode otherwise, the same for a number, the optional costs, a division and a payment
/// (the lowest number, no cost, an even division and the engine's own payment otherwise), the first cards of the list
/// for a choice of cards, of the hand for a discard or a bottoming, the order given for an order of cards, keeping the
/// hand and lethal damage to each blocker in turn.
///
/// The answer is read from the decision alone. `None` for a decision that has no answer to give (no option, no blocker
/// to take the damage, a payment only the player can make): the clock stops and the decision waits.
pub(crate) fn fallback(decision: &Decision) -> Option<Answer> {
    match &decision.kind {
        DecisionKind::Priority(_) => Some(pass()),
        DecisionKind::DeclareBlockers(options) => {
            Some(Answer::DeclareBlockers(options.required_blocks()))
        }
        DecisionKind::ChooseTargets(options) => {
            Some(Answer::ChooseTargets(if options.can_cancel {
                TargetChoice::Cancel
            } else {
                TargetChoice::Remove
            }))
        }
        DecisionKind::OrderTriggers(options) => Some(Answer::OrderTriggers(TriggersChoice(
            (0..options.triggers.len()).collect(),
        ))),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(false))),
        // The first effect on offer, which the rules let the player choose; the effects that are left apply after it.
        DecisionKind::ChooseReplacement(options) => (!options.candidates.is_empty())
            .then_some(Answer::ChooseReplacement(ReplacementChoice(0))),
        // A list with nothing in it has no answer to give; the engine does not ask one.
        DecisionKind::ChooseOption(options) => {
            (!options.options.is_empty()).then_some(Answer::ChooseOption(OptionChoice(0)))
        }
        DecisionKind::ChooseMode(options) => first_modes(options).map(Answer::ChooseMode),
        // A cast is taken back when the options allow it; otherwise the lowest number, no optional cost and an even
        // division, the answers that decide the least.
        DecisionKind::ChooseNumber(options) => Some(Answer::ChooseNumber(if options.can_cancel {
            NumberChoice::Cancel
        } else {
            NumberChoice::Number(options.min)
        })),
        DecisionKind::ChooseCosts(options) => Some(Answer::ChooseCosts(if options.can_cancel {
            CostsChoice::Cancel
        } else {
            CostsChoice::Costs(Vec::new())
        })),
        DecisionKind::DivideAmount(options) => even_division(options).map(Answer::DivideAmount),
        // A cost that takes any number of creatures for their power (crew) is taken back, as the cast of a spell is:
        // the first creature alone may not have the power.
        DecisionKind::ChooseCards(options) if options.can_cancel && options.max > options.min => {
            Some(Answer::ChooseCards(CardsChoice(Vec::new())))
        }
        DecisionKind::ChooseCards(options) => Some(Answer::ChooseCards(first_cards(options))),
        DecisionKind::OrderCards(options) => {
            Some(Answer::OrderCards(OrderChoice(options.cards.clone())))
        }
        DecisionKind::Mulligan(_) => Some(Answer::Mulligan(MulliganChoice::Keep)),
        DecisionKind::BottomCards(options) => Some(Answer::BottomCards(first_of_hand(
            &options.hand,
            options.count,
        ))),
        DecisionKind::PayMana(options) => pay_fallback(options).map(Answer::PayMana),
        DecisionKind::DeclareAttackers(options) => {
            Some(Answer::DeclareAttackers(options.required_attack()))
        }
        DecisionKind::AssignDamage(options) => in_order(options).map(Answer::AssignDamage),
        DecisionKind::Discard(options) => {
            Some(Answer::Discard(first_of_hand(&options.hand, options.count)))
        }
    }
}

/// The first `count` cards of the hand in the order it was offered.
fn first_of_hand(hand: &[ObjectId], count: usize) -> Vec<ObjectId> {
    hand.iter().copied().take(count).collect()
}

/// Backing out of the cast when the options allow it, else the engine's own payment. `None` when neither exists: the
/// seat has to pay by hand.
fn pay_fallback(options: &PayOptions) -> Option<PayChoice> {
    if options.can_cancel {
        Some(PayChoice::Cancel)
    } else {
        options.auto_available.then_some(PayChoice::Auto)
    }
}

/// Lethal damage to each blocker in the order offered, the rest to the last one, or to the player attacked when
/// trampling. `None` when there is no blocker to take the damage.
fn in_order(options: &DamageOptions) -> Option<Vec<(ObjectId, u32)>> {
    let mut left = options.power;
    let mut shares: Vec<(ObjectId, u32)> = options
        .blockers
        .iter()
        .enumerate()
        .map(|(at, blocker)| {
            let share = options
                .lethal
                .get(at)
                .map_or(left, |lethal| left.min(*lethal));
            left -= share;
            (*blocker, share)
        })
        .collect();
    if options.excess.is_none() {
        shares.last_mut()?.1 += left;
    }
    Some(shares)
}

/// The first `min` cards of the list in the order it was offered, all of them when fewer are offered. `min <= max`, so
/// the answer never has more than the most.
fn first_cards(options: &CardsOptions) -> CardsChoice {
    let wanted = usize::try_from(options.min).unwrap_or(usize::MAX);
    CardsChoice(options.from.iter().copied().take(wanted).collect())
}

/// Backing out of the cast when the options allow it, else the amount shared as evenly as it goes: one at least to each
/// target, the first getting what is left over. `None` when the amount is smaller than the number of targets.
fn even_division(options: &DivideOptions) -> Option<DivideChoice> {
    if options.can_cancel {
        return Some(DivideChoice::Cancel);
    }
    let count = u32::try_from(options.among.len())
        .ok()
        .filter(|count| *count > 0)?;
    let share = options.total / count;
    if share == 0 {
        return None;
    }
    let shares: Vec<(Target, u32)> = options
        .among
        .iter()
        .enumerate()
        .map(|(at, target)| {
            let amount = if at == 0 {
                options.total - share * (count - 1)
            } else {
                share
            };
            (*target, amount)
        })
        .collect();
    Some(DivideChoice::Division(shares))
}

/// Backing out of the cast when the options allow it, else the first modes of the list: one, or as many as the
/// choice has to have at least. When the list is shorter than that, a choice that may repeat a mode repeats the first.
fn first_modes(options: &ModeOptions) -> Option<ModeChoice> {
    if options.can_cancel {
        return Some(ModeChoice::Cancel);
    }
    let first = options.modes.first()?;
    let wanted = usize::try_from(options.min.max(1)).unwrap_or(1);
    let mut chosen: Vec<usize> = options
        .modes
        .iter()
        .take(wanted)
        .map(|mode| mode.index)
        .collect();
    if chosen.len() < wanted {
        if !options.repeat {
            return None;
        }
        chosen.resize(wanted, first.index);
    }
    Some(ModeChoice::Modes(chosen))
}

/// Whether `answer` takes a triggered ability off the stack without a target. Only the clock gives it ([`fallback`]):
/// a player with a legal target has to choose one.
pub(crate) fn is_removal(answer: &Answer) -> bool {
    matches!(answer, Answer::ChooseTargets(TargetChoice::Remove))
}

impl<S: EventStore> GameActor<S> {
    /// When the pending decision's clock runs out, if it has one.
    pub(crate) fn due(&self) -> Option<Instant> {
        match &self.phase {
            Phase::Running(running) => running.due(),
            Phase::Lobby { .. } => None,
        }
    }

    /// Answers for the seat whose clock ran out, with a pass or no blocks, as if the seat had, unless the seat holds an
    /// extension: that is spent instead, the deadline moves and the table is told the new bank. The same step passes for
    /// a seat that cannot act once its dwell is over: the decision was never sent, and the pass is saved, projected and
    /// sent like the seat's own, as a batch of its own.
    pub(crate) async fn expire(&mut self) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        let sent = running.sent_decision();
        if let Some(extended) = running.extend() {
            if let Some((seat, kind)) = sent {
                tracing::info!(game = %self.id, seat = seat.index(), kind, bank = extended.bank, "clock ran out; an extension was spent");
            }
            self.announce_extension(extended);
            return Ok(());
        }
        if sent.is_some() {
            running.clock_ran_out();
        }
        if let Ok(step) = running.fall_back() {
            if let Some((seat, kind)) = sent {
                tracing::info!(game = %self.id, seat = seat.index(), kind, "clock ran out; the table answered for the seat");
            }
            // If the move cannot be saved the decision stays; trying again at once would spin.
            running.postpone(Instant::now() + RETRY_AFTER);
            self.publish(step).await
        } else {
            running.stop_clock();
            Ok(())
        }
    }

    /// Tells the table that `extended`'s seat spent an extension (its bank, to every seat) and the seat its decision
    /// again with the time it now has, as a reconnect is told what is left.
    fn announce_extension(&mut self, extended: Extended) {
        let Extended {
            id,
            decision,
            deadline_ms,
            bank,
        } = extended;
        let seat = decision.player;
        self.seats.broadcast(&ServerMessage::Bank { seat, bank });
        self.seats.send(
            seat,
            ServerMessage::Decision {
                id,
                decision,
                deadline_ms: Some(deadline_ms),
                show_after_ms: None,
            },
        );
    }

    /// Starts `seat`'s yield; if the seat faces a priority decision it is passed at once.
    ///
    /// The yield, the passes it makes and their save are one unit, like an answer and the auto-passes after it: a
    /// failed save keeps no yield. A yield that ends at once (the stack it waits on is empty, or the seat holds a
    /// stop here) passes nothing and the decision is sent again with a new id, so the seat is never left without the
    /// prompt it owes an answer to. A seat in Full Control cannot yield: it is asked at every window.
    pub(crate) async fn set_yield(
        &mut self,
        seat: PlayerId,
        until: YieldUntil,
    ) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        let state = running.engine.state();
        if running.engine.result().is_some() || !state.is_alive(seat) {
            let reason = "you are not in a game that is still being played";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        }
        if self.stops.full_control(seat) != FullControlMode::Off {
            let reason =
                "full control is on, so the server cannot pass for you; turn it off to yield";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        }
        let mut yields = self.yields.clone();
        if !running.asks_priority_of(seat) {
            yields.start(seat, until, state);
            self.commit_yields(yields);
            return Ok(());
        }
        // The seat asks while it looks at a window, which a stop of its own there does not take back.
        yields.start_facing(seat, until, state);
        match running.restart_decision(seat) {
            Ok(step) => {
                let stops = self.stops.clone();
                self.publish_with(step, yields, stops).await
            }
            Err(rejection) => {
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Ends `seat`'s yield; the seat is told if it had one.
    pub(crate) fn cancel_yield(&mut self, seat: PlayerId) {
        let mut yields = self.yields.clone();
        if yields.cancel(seat) {
            self.commit_yields(yields);
        }
    }

    /// Takes `yields` as the seats' yields and tells each seat whose yield changed.
    pub(crate) fn commit_yields(&mut self, yields: Yields) {
        for (seat, until) in self.yields.replace(yields) {
            self.seats.send(seat, ServerMessage::Yielding { until });
        }
    }

    /// Settles what a game that is starting or resumed from its log left pending: if the windows pass for the seat
    /// it waits on, it is passed for before any client connects. A seat that must wait at its window gets a dwell that
    /// starts now, like one that meets such a window while the game runs.
    pub(crate) async fn start_up(&mut self) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        let Some(seat) = running.waiting_on() else {
            return Ok(());
        };
        match running.verdict_pending(seat, &self.stops, &self.yields) {
            Some(Verdict::Pass) => match running.fall_back() {
                Ok(step) => self.publish(step).await,
                Err(rejection) => {
                    self.refuse(seat, rejection);
                    Ok(())
                }
            },
            Some(Verdict::Dwell) => {
                running.settle(false, Halt::Dwell);
                Ok(())
            }
            Some(Verdict::Ask) | None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the pace rules give the clock, the safe default and the priority test exactly to the decisions they are meant for, whatever the settings, and a seat starts in the mode its auto-pass default names.
    use super::*;
    use mtg_core::ids::ObjectId;
    use mtg_engine::choose::{
        CardsOptions, ChoiceOption, CostsOptions, ModeOption, NumberOptions, OptionOptions,
        OptionalCost, OrderOptions, ReplacementCandidate, ReplacementOptions, TargetOptions,
        TriggerEntry, TriggersOptions, YesNoOptions,
    };
    use mtg_engine::combat::{AttackTarget, DamageTarget};
    use mtg_engine::decision::{
        AttackOptions, AttackerOption, BlockOptions, BottomOptions, DiscardOptions,
        MulliganOptions, PriorityOptions,
    };
    use mtg_engine::mana::ManaPool;
    use mtg_engine::object::ObjectRef;
    use mtg_engine::spend::Spend;
    use rstest::rstest;

    const ACTIVE: PlayerId = PlayerId(0);
    const OTHER: PlayerId = PlayerId(1);

    fn priority(can_act: bool) -> DecisionKind {
        DecisionKind::Priority(Box::new(PriorityOptions {
            playable_lands: Vec::new(),
            castable: Vec::new(),
            mana_abilities: Vec::new(),
            abilities: Vec::new(),
            suspendable: Vec::new(),
            foretellable: Vec::new(),
            faces: Vec::new(),
            can_act,
            can_pass: true,
        }))
    }

    fn blockers() -> DecisionKind {
        DecisionKind::DeclareBlockers(BlockOptions {
            blockers: Vec::new(),
            min_blockers: Vec::new(),
            required: Vec::new(),
        })
    }

    fn mulligan() -> DecisionKind {
        DecisionKind::Mulligan(MulliganOptions {
            hand: Vec::new(),
            mulligans_taken: 0,
            can_mulligan: true,
            free_mulligans: 0,
            bottom_if_keep: 0,
            bottom_if_mulligan: 1,
            hand_if_mulligan: None,
            scry_if_keep: None,
            scry_if_mulligan: None,
        })
    }

    /// A hand of `size` cards of which `count` go to the bottom.
    fn bottoming(size: usize, count: usize) -> DecisionKind {
        DecisionKind::BottomCards(BottomOptions {
            hand: offered(size),
            count,
        })
    }

    fn payment(can_cancel: bool, auto_available: bool) -> DecisionKind {
        DecisionKind::PayMana(PayOptions {
            spend: Box::new(Spend::other()),
            spell: ObjectId(1),
            cost: mtg_core::mana::ManaCost::parse("{1}").unwrap(),
            pool: ManaPool::default(),
            sources: Vec::new(),
            auto_available,
            can_cancel,
            life: 20,
            max_life: 0,
            help: Box::default(),
        })
    }

    /// Creatures 10, 11 and 12 may attack the player 0; those named in `required` have to.
    fn attackers(required: &[u64]) -> DecisionKind {
        DecisionKind::DeclareAttackers(AttackOptions {
            attackers: (10..13)
                .map(|n| {
                    AttackerOption::free(ObjectId(n), vec![AttackTarget::Player(PlayerId(0))])
                })
                .collect(),
            required: required.iter().copied().map(ObjectId).collect(),
            mana: 0,
        })
    }

    /// An attacker of `power` blocked by creatures 20, 21, ... whose lethal damage is `lethal`, trampling over them
    /// to the player 0 when `trample`.
    fn damage(power: u32, lethal: &[u32], trample: bool) -> DecisionKind {
        DecisionKind::AssignDamage(DamageOptions {
            attacker: ObjectId(1),
            power,
            blockers: (20..).map(ObjectId).take(lethal.len()).collect(),
            excess: trample.then_some(DamageTarget::Player(PlayerId(0))),
            lethal: lethal.to_vec(),
        })
    }

    /// The answer that gives creatures 20, 21, ... these shares.
    fn assigned(shares: &[u32]) -> Answer {
        Answer::AssignDamage((20..).map(ObjectId).zip(shares.iter().copied()).collect())
    }

    fn discard(size: usize, count: usize) -> DecisionKind {
        DecisionKind::Discard(DiscardOptions {
            hand: offered(size),
            count,
        })
    }

    /// The kinds the think clock runs, for the seat that is not the active player.
    fn think_kinds() -> Vec<DecisionKind> {
        vec![
            mulligan(),
            bottoming(7, 1),
            payment(true, true),
            payment(false, true),
            attackers(&[]),
            damage(3, &[2, 2], false),
            discard(8, 1),
        ]
    }

    /// The kinds that are asked before any turn exists, whose clock does not look at the active player.
    fn opening_kinds() -> Vec<DecisionKind> {
        vec![mulligan(), bottoming(7, 1)]
    }

    fn targets(can_cancel: bool) -> DecisionKind {
        DecisionKind::ChooseTargets(TargetOptions {
            source: ObjectId(1),
            spec: 0,
            legal: Vec::new(),
            min: 1,
            max: 1,
            can_cancel,
            wording: String::new(),
            triggered: !can_cancel,
        })
    }

    fn order(count: usize) -> DecisionKind {
        DecisionKind::OrderTriggers(TriggersOptions {
            triggers: (0..count)
                .map(|n| TriggerEntry {
                    source: ObjectRef {
                        id: ObjectId(n as u64),
                        counter: 0,
                    },
                    ability: 0,
                    when: String::new(),
                })
                .collect(),
        })
    }

    /// `count` replacement effects that apply to one event.
    fn replacements(count: usize) -> DecisionKind {
        DecisionKind::ChooseReplacement(ReplacementOptions {
            candidates: (0..count)
                .map(|n| ReplacementCandidate {
                    source: ObjectId(n as u64 + 1),
                    ability: 0,
                    shield: None,
                })
                .collect(),
            text: "Create 2 token(s).".into(),
        })
    }

    fn question() -> DecisionKind {
        DecisionKind::YesNo(YesNoOptions::new(ObjectId(1), "Pay {1}?"))
    }

    /// The same question put to a player who cannot pay: No is the only answer.
    fn unaffordable() -> DecisionKind {
        DecisionKind::YesNo(YesNoOptions {
            only_no: true,
            ..YesNoOptions::new(ObjectId(1), "Pay {1}?")
        })
    }

    fn options(count: usize) -> DecisionKind {
        DecisionKind::ChooseOption(OptionOptions {
            source: ObjectId(1),
            options: (0..count)
                .map(|n| ChoiceOption::Label(format!("Option {n}")))
                .collect(),
            text: String::new(),
        })
    }

    /// `count` modes on offer, whose indices (1, 3, 5, ...) differ from their places in the list.
    fn modes(count: usize, min: u32, max: u32, repeat: bool, can_cancel: bool) -> DecisionKind {
        DecisionKind::ChooseMode(ModeOptions {
            source: ObjectId(1),
            modes: (0..count)
                .map(|n| ModeOption {
                    index: 2 * n + 1,
                    label: None,
                })
                .collect(),
            min,
            max,
            repeat,
            can_cancel,
        })
    }

    fn number(min: u32, max: Option<u32>, can_cancel: bool) -> DecisionKind {
        DecisionKind::ChooseNumber(NumberOptions {
            source: ObjectId(1),
            min,
            max,
            except: Vec::new(),
            text: "Choose a value for X.".into(),
            can_cancel,
        })
    }

    fn costs(count: usize, can_cancel: bool) -> DecisionKind {
        DecisionKind::ChooseCosts(CostsOptions {
            source: ObjectId(1),
            optional: (0..count)
                .map(|index| OptionalCost {
                    index,
                    label: format!("Kicker {index}"),
                })
                .collect(),
            can_cancel,
        })
    }

    fn division(total: u32, count: u64, can_cancel: bool) -> DecisionKind {
        DecisionKind::DivideAmount(DivideOptions {
            source: ObjectId(1),
            total,
            among: (0..count).map(|n| Target::Player(PlayerId(n))).collect(),
            text: String::new(),
            can_cancel,
        })
    }

    /// The answer that gives the first seats these shares, in order.
    fn shares(given: &[u32]) -> Answer {
        Answer::DivideAmount(DivideChoice::Division(
            given
                .iter()
                .zip(0u64..)
                .map(|(share, n)| (Target::Player(PlayerId(n)), *share))
                .collect(),
        ))
    }

    /// `count` cards on offer whose ids run down, so the first cards offered are not the ones with the lowest ids.
    fn offered(count: usize) -> Vec<ObjectId> {
        (0..count).map(|n| ObjectId(100 - 3 * n as u64)).collect()
    }

    fn cards(count: usize, min: u32, max: u32) -> DecisionKind {
        DecisionKind::ChooseCards(CardsOptions {
            source: ObjectId(1),
            from: offered(count),
            min,
            max,
            text: String::new(),
            can_cancel: false,
        })
    }

    fn ordering(count: usize) -> DecisionKind {
        DecisionKind::OrderCards(OrderOptions {
            source: ObjectId(1),
            cards: offered(count),
            text: String::new(),
        })
    }

    /// The cards with these ids.
    fn ids(ids: &[u64]) -> Vec<ObjectId> {
        ids.iter().copied().map(ObjectId).collect()
    }

    /// The answer that chooses `ids`.
    fn chosen(ids: &[u64]) -> Answer {
        Answer::ChooseCards(CardsChoice(ids.iter().copied().map(ObjectId).collect()))
    }

    /// The answer that puts the cards in the order `ids`.
    fn ordered(ids: &[u64]) -> Answer {
        Answer::OrderCards(OrderChoice(ids.iter().copied().map(ObjectId).collect()))
    }

    fn decision(player: PlayerId, kind: DecisionKind) -> Decision {
        Decision { player, kind }
    }

    fn pace() -> Pace {
        Pace {
            auto_pass: true,
            response: Some(Duration::from_secs(10)),
            blocks: Some(Duration::from_secs(45)),
            think: Some(Duration::from_secs(60)),
            active: None,
            extension: Some(Duration::from_secs(20)),
            grace: Duration::from_secs(3),
            dwell: Dwell::OFF,
        }
    }

    #[rstest]
    #[case::response_to_an_opponents_turn(OTHER, priority(true), Some(10))]
    #[case::nothing_to_respond_with_is_hurried_too(OTHER, priority(false), Some(10))]
    #[case::the_active_players_own_priority(ACTIVE, priority(true), None)]
    #[case::the_active_player_with_nothing_to_do(ACTIVE, priority(false), None)]
    #[case::a_defender_blocking(OTHER, blockers(), Some(45))]
    #[case::choosing_the_targets_of_a_spell(OTHER, targets(true), Some(10))]
    #[case::choosing_the_targets_of_a_trigger(OTHER, targets(false), Some(10))]
    #[case::ordering_triggers(OTHER, order(2), Some(10))]
    #[case::the_active_player_choosing_targets(ACTIVE, targets(true), None)]
    #[case::the_active_player_choosing_the_targets_of_a_trigger(ACTIVE, targets(false), None)]
    #[case::the_active_player_ordering_triggers(ACTIVE, order(2), None)]
    #[case::answering_a_question(OTHER, question(), Some(10))]
    #[case::answering_a_payment_that_cannot_be_made(OTHER, unaffordable(), Some(10))]
    #[case::choosing_an_option(OTHER, options(3), Some(10))]
    #[case::choosing_the_modes_of_a_spell(OTHER, modes(3, 1, 1, false, true), Some(10))]
    #[case::choosing_the_modes_of_a_trigger(OTHER, modes(3, 1, 1, false, false), Some(10))]
    #[case::choosing_x(OTHER, number(0, Some(3), true), Some(10))]
    #[case::choosing_a_number_for_a_trigger(OTHER, number(0, None, false), Some(10))]
    #[case::choosing_optional_costs(OTHER, costs(2, true), Some(10))]
    #[case::dividing_an_amount(OTHER, division(3, 2, true), Some(10))]
    #[case::the_active_player_choosing_x(ACTIVE, number(0, Some(3), true), None)]
    #[case::the_active_player_choosing_optional_costs(ACTIVE, costs(2, true), None)]
    #[case::the_active_player_dividing_an_amount(ACTIVE, division(3, 2, true), None)]
    #[case::the_active_player_answering_a_question(ACTIVE, question(), None)]
    #[case::the_active_player_answering_a_payment_that_cannot_be_made(ACTIVE, unaffordable(), None)]
    #[case::the_active_player_choosing_an_option(ACTIVE, options(3), None)]
    #[case::the_active_player_choosing_modes(ACTIVE, modes(3, 1, 1, false, true), None)]
    #[case::choosing_a_card(OTHER, cards(3, 1, 1), Some(10))]
    #[case::choosing_up_to_some_cards(OTHER, cards(5, 0, 3), Some(10))]
    #[case::choosing_from_nothing(OTHER, cards(0, 0, 1), Some(10))]
    #[case::putting_cards_in_order(OTHER, ordering(3), Some(10))]
    #[case::ordering_replacement_effects(OTHER, replacements(2), Some(10))]
    #[case::the_active_player_ordering_replacement_effects(ACTIVE, replacements(2), None)]
    #[case::putting_one_card_in_order(OTHER, ordering(1), Some(10))]
    #[case::the_active_player_choosing_cards(ACTIVE, cards(3, 1, 1), None)]
    #[case::the_active_player_putting_cards_in_order(ACTIVE, ordering(3), None)]
    #[case::keeping_or_mulliganing(OTHER, mulligan(), Some(60))]
    #[case::the_first_player_keeping_or_mulliganing(ACTIVE, mulligan(), Some(60))]
    #[case::bottoming_after_a_keep(OTHER, bottoming(7, 1), Some(60))]
    #[case::the_first_player_bottoming(ACTIVE, bottoming(7, 1), Some(60))]
    #[case::paying_for_a_spell(OTHER, payment(true, true), Some(60))]
    #[case::paying_for_a_cost_that_cannot_be_taken_back(OTHER, payment(false, true), Some(60))]
    #[case::the_active_player_paying(ACTIVE, payment(true, true), None)]
    #[case::dividing_combat_damage(OTHER, damage(3, &[2, 2], false), Some(60))]
    #[case::the_active_player_dividing_combat_damage(ACTIVE, damage(3, &[2, 2], false), None)]
    #[case::declaring_attackers(OTHER, attackers(&[]), Some(60))]
    #[case::the_active_player_declaring_attackers(ACTIVE, attackers(&[]), None)]
    #[case::discarding(OTHER, discard(8, 1), Some(60))]
    #[case::the_active_player_discarding_to_hand_size(ACTIVE, discard(8, 1), None)]
    fn clocks_by_decision(
        #[case] player: PlayerId,
        #[case] kind: DecisionKind,
        #[case] seconds: Option<u64>,
    ) {
        let got = pace().clock_for(&decision(player, kind), ACTIVE);
        assert_eq!(got, seconds.map(Duration::from_secs));
    }

    #[test]
    fn the_opening_clock_is_the_think_clock_whatever_the_decision() {
        assert_eq!(pace().opening_clock(), Some(Duration::from_secs(60)));
        let none = Pace {
            think: None,
            ..pace()
        };
        assert_eq!(none.opening_clock(), None);
    }

    #[test]
    fn the_opening_decisions_run_on_the_think_clock_for_every_seat() {
        for kind in opening_kinds() {
            for player in [ACTIVE, OTHER, PlayerId(2), PlayerId(3)] {
                let d = decision(player, kind.clone());
                assert_eq!(
                    pace().clock_for(&d, ACTIVE),
                    Some(Duration::from_secs(60)),
                    "{d:?}"
                );
            }
        }
    }

    #[test]
    fn after_the_opening_the_think_clock_hurries_a_seat_that_is_not_the_active_player_only() {
        for kind in think_kinds() {
            let (other, active) = (
                decision(OTHER, kind.clone()),
                decision(ACTIVE, kind.clone()),
            );
            assert_eq!(
                pace().clock_for(&other, ACTIVE),
                Some(Duration::from_secs(60)),
                "{other:?}"
            );
            let opening = opening_kinds().contains(&kind);
            assert_eq!(
                pace().clock_for(&active, ACTIVE).is_some(),
                opening,
                "{active:?}"
            );
        }
    }

    #[test]
    fn without_a_think_setting_no_decision_that_takes_thought_has_a_clock() {
        for kind in think_kinds() {
            let none = Pace {
                think: None,
                ..pace()
            };
            for player in [ACTIVE, OTHER] {
                let d = decision(player, kind.clone());
                assert_eq!(none.clock_for(&d, ACTIVE), None, "{d:?}");
            }
        }
    }

    #[rstest]
    #[case::manual(Pace::MANUAL)]
    #[case::response_off(Pace { response: None, think: None, ..pace() })]
    fn a_missing_setting_means_no_clock(#[case] pace: Pace) {
        assert_eq!(
            pace.clock_for(&decision(OTHER, priority(true)), ACTIVE),
            None
        );
        assert_eq!(
            pace.clock_for(&decision(OTHER, targets(true)), ACTIVE),
            None
        );
        assert_eq!(pace.clock_for(&decision(OTHER, order(2)), ACTIVE), None);
        for kind in [
            question(),
            unaffordable(),
            replacements(2),
            options(2),
            modes(2, 1, 1, false, true),
            number(0, Some(1), true),
            costs(1, true),
            division(2, 2, true),
            cards(2, 1, 1),
            ordering(2),
        ] {
            assert_eq!(pace.clock_for(&decision(OTHER, kind), ACTIVE), None);
        }
        if pace.blocks.is_none() {
            assert_eq!(pace.clock_for(&decision(OTHER, blockers()), ACTIVE), None);
        }
        if pace.think.is_none() {
            for kind in think_kinds() {
                assert_eq!(pace.clock_for(&decision(OTHER, kind), ACTIVE), None);
            }
        }
    }

    #[test]
    fn the_defaults_are_ten_forty_five_and_sixty_seconds_and_a_dwell_with_auto_pass_on() {
        let pace = Pace::default();
        assert!(pace.auto_pass);
        assert_eq!(pace.response, Some(Duration::from_secs(10)));
        assert_eq!(pace.blocks, Some(Duration::from_secs(45)));
        assert_eq!(pace.think, Some(Duration::from_secs(60)));
        assert_eq!(pace.active, None, "the active seat is not hurried");
        assert_eq!(pace.extension, Some(Duration::from_secs(20)));
        assert_eq!(pace.grace, Duration::from_secs(3));
        assert_eq!(
            pace.dwell.range(),
            (Duration::from_millis(250), Duration::from_millis(700))
        );
        assert_eq!(Pace::MANUAL.response, None);
        assert!(!std::hint::black_box(Pace::MANUAL).auto_pass);
        assert!(Pace::MANUAL.dwell.is_off(), "the manual pace never waits");
    }

    #[rstest]
    #[case::pass_for_a_priority_holder(priority(true), Some(pass()))]
    #[case::pass_even_when_idle(priority(false), Some(pass()))]
    #[case::no_blocks(blockers(), Some(Answer::DeclareBlockers(Vec::new())))]
    #[case::a_spell_is_taken_back(targets(true), Some(Answer::ChooseTargets(TargetChoice::Cancel)))]
    #[case::a_trigger_is_removed(targets(false), Some(Answer::ChooseTargets(TargetChoice::Remove)))]
    #[case::triggers_go_on_in_the_order_they_triggered(order(3), Some(Answer::OrderTriggers(TriggersChoice(vec![0, 1, 2]))))]
    #[case::no_to_a_question(question(), Some(Answer::YesNo(YesNoChoice(false))))]
    #[case::no_to_a_payment_that_cannot_be_made(
        unaffordable(),
        Some(Answer::YesNo(YesNoChoice(false)))
    )]
    #[case::the_first_replacement_effect(
        replacements(3),
        Some(Answer::ChooseReplacement(ReplacementChoice(0)))
    )]
    #[case::no_replacement_effect_to_choose(replacements(0), None)]
    #[case::the_first_of_the_options(options(4), Some(Answer::ChooseOption(OptionChoice(0))))]
    #[case::the_only_option(options(1), Some(Answer::ChooseOption(OptionChoice(0))))]
    #[case::no_option_to_choose(options(0), None)]
    #[case::a_spell_is_taken_back_instead_of_choosing_modes(
        modes(3, 1, 1, false, true),
        Some(Answer::ChooseMode(ModeChoice::Cancel))
    )]
    #[case::taking_it_back_beats_a_choice_of_several(
        modes(4, 2, 2, false, true),
        Some(Answer::ChooseMode(ModeChoice::Cancel))
    )]
    #[case::a_trigger_takes_the_first_mode(modes(3, 1, 1, false, false), Some(Answer::ChooseMode(ModeChoice::Modes(vec![1]))))]
    #[case::the_index_of_the_mode_and_not_its_place(modes(1, 1, 1, false, false), Some(Answer::ChooseMode(ModeChoice::Modes(vec![1]))))]
    #[case::a_choice_of_two_takes_the_first_two(modes(4, 2, 2, false, false), Some(Answer::ChooseMode(ModeChoice::Modes(vec![1, 3]))))]
    #[case::up_to_three_takes_one(modes(4, 0, 3, false, false), Some(Answer::ChooseMode(ModeChoice::Modes(vec![1]))))]
    #[case::a_repeating_choice_with_too_few_modes_repeats_the_first(modes(1, 3, 3, true, false), Some(Answer::ChooseMode(ModeChoice::Modes(vec![1, 1, 1]))))]
    #[case::too_few_modes_that_may_not_repeat_have_no_answer(modes(1, 2, 2, false, false), None)]
    #[case::no_modes_to_choose(modes(0, 1, 1, false, false), None)]
    #[case::x_is_taken_back(
        number(0, Some(3), true),
        Some(Answer::ChooseNumber(NumberChoice::Cancel))
    )]
    #[case::the_lowest_number_when_it_cannot_be_taken_back(
        number(2, None, false),
        Some(Answer::ChooseNumber(NumberChoice::Number(2)))
    )]
    #[case::optional_costs_are_taken_back(
        costs(2, true),
        Some(Answer::ChooseCosts(CostsChoice::Cancel))
    )]
    #[case::no_optional_cost_when_it_cannot_be_taken_back(costs(2, false), Some(Answer::ChooseCosts(CostsChoice::Costs(vec![]))))]
    #[case::a_division_is_taken_back(
        division(5, 2, true),
        Some(Answer::DivideAmount(DivideChoice::Cancel))
    )]
    #[case::an_even_division_the_first_gets_the_rest(division(5, 2, false), Some(shares(&[3, 2])))]
    #[case::an_even_division_of_three_ways(division(6, 3, false), Some(shares(&[2, 2, 2])))]
    #[case::a_division_with_one_each(division(2, 2, false), Some(shares(&[1, 1])))]
    #[case::a_division_that_cannot_give_one_each_has_no_answer(division(1, 2, false), None)]
    #[case::the_first_card_offered(cards(4, 1, 1), Some(chosen(&[100])))]
    #[case::the_first_two_cards_not_the_lowest_ids(cards(4, 2, 2), Some(chosen(&[100, 97])))]
    #[case::at_least_one_takes_one(cards(4, 1, 3), Some(chosen(&[100])))]
    #[case::at_least_two_takes_two(cards(5, 2, 5), Some(chosen(&[100, 97])))]
    #[case::up_to_some_takes_none(cards(4, 0, 3), Some(chosen(&[])))]
    #[case::any_number_takes_none(cards(4, 0, 4), Some(chosen(&[])))]
    #[case::all_of_them_when_all_are_wanted(cards(3, 3, 3), Some(chosen(&[100, 97, 94])))]
    #[case::all_of_the_cards_when_fewer_are_offered_than_wanted(cards(2, 3, 3), Some(chosen(&[100, 97])))]
    #[case::nothing_to_choose_from_when_none_is_wanted(cards(0, 0, 2), Some(chosen(&[])))]
    #[case::nothing_to_choose_from_when_one_is_wanted(cards(0, 1, 1), Some(chosen(&[])))]
    #[case::the_order_given(ordering(3), Some(ordered(&[100, 97, 94])))]
    #[case::one_card_is_its_own_order(ordering(1), Some(ordered(&[100])))]
    #[case::no_cards_are_an_empty_order(ordering(0), Some(ordered(&[])))]
    #[case::keep_the_hand(mulligan(), Some(Answer::Mulligan(MulliganChoice::Keep)))]
    #[case::bottom_the_first_card_of_the_hand(bottoming(7, 1), Some(Answer::BottomCards(ids(&[100]))))]
    #[case::bottom_the_first_two_cards(bottoming(7, 2), Some(Answer::BottomCards(ids(&[100, 97]))))]
    #[case::bottom_nothing_when_nothing_is_asked(bottoming(7, 0), Some(Answer::BottomCards(ids(&[]))))]
    #[case::a_cast_is_taken_back_instead_of_paid(
        payment(true, true),
        Some(Answer::PayMana(PayChoice::Cancel))
    )]
    #[case::a_cast_is_taken_back_with_no_payment_at_hand(
        payment(true, false),
        Some(Answer::PayMana(PayChoice::Cancel))
    )]
    #[case::the_engine_pays_what_cannot_be_taken_back(
        payment(false, true),
        Some(Answer::PayMana(PayChoice::Auto))
    )]
    #[case::a_payment_only_the_player_can_make_has_no_answer(payment(false, false), None)]
    #[case::no_attack_when_none_is_required(attackers(&[]), Some(Answer::DeclareAttackers(Vec::new())))]
    #[case::only_the_attackers_a_rule_requires(
        attackers(&[12, 10]),
        Some(Answer::DeclareAttackers(vec![
            (ObjectId(10), AttackTarget::Player(PlayerId(0))),
            (ObjectId(12), AttackTarget::Player(PlayerId(0))),
        ]))
    )]
    #[case::lethal_damage_to_the_first_and_the_rest_to_the_last(damage(3, &[2, 2], false), Some(assigned(&[2, 1])))]
    #[case::too_little_damage_for_the_second_blocker(damage(1, &[2, 2], false), Some(assigned(&[1, 0])))]
    #[case::the_rest_goes_to_the_last_blocker(damage(5, &[2, 2], false), Some(assigned(&[2, 3])))]
    #[case::one_blocker_takes_everything(damage(7, &[3], false), Some(assigned(&[7])))]
    #[case::three_blockers_in_turn(damage(2, &[1, 1, 1], false), Some(assigned(&[1, 1, 0])))]
    #[case::a_trampler_keeps_the_rest_for_the_player(damage(5, &[2, 2], true), Some(assigned(&[2, 2])))]
    #[case::a_trampler_short_of_lethal_gives_all_it_has(damage(3, &[2, 2], true), Some(assigned(&[2, 1])))]
    #[case::a_trampler_with_nothing_blocking_it_gives_nothing(damage(4, &[], true), Some(assigned(&[])))]
    #[case::no_blocker_to_give_the_damage_to(damage(4, &[], false), None)]
    #[case::discard_the_first_card_of_the_hand(discard(8, 1), Some(Answer::Discard(ids(&[100]))))]
    #[case::discard_the_first_two_cards(discard(9, 2), Some(Answer::Discard(ids(&[100, 97]))))]
    fn safe_defaults(#[case] kind: DecisionKind, #[case] expected: Option<Answer>) {
        assert_eq!(fallback(&decision(OTHER, kind)), expected);
    }

    #[test]
    fn the_clock_and_the_fallback_are_the_same_for_every_seat_and_every_shape_of_options() {
        let kinds = [
            question(),
            unaffordable(),
            replacements(2),
            options(1),
            options(11),
            modes(1, 1, 1, false, true),
            modes(5, 2, 3, true, false),
            number(0, None, false),
            number(3, Some(40), true),
            costs(3, true),
            costs(1, false),
            division(4, 2, true),
            division(9, 3, false),
            cards(1, 1, 1),
            cards(12, 0, 12),
            cards(5, 2, 3),
            cards(0, 0, 1),
            ordering(1),
            ordering(7),
        ];
        for kind in kinds {
            let want = (
                pace().clock_for(&decision(OTHER, kind.clone()), ACTIVE),
                fallback(&decision(OTHER, kind.clone())),
            );
            assert_eq!(want.0, Some(Duration::from_secs(10)), "{kind:?}");
            for seat in 1..12 {
                let d = decision(PlayerId::from_index(seat), kind.clone());
                assert_eq!((pace().clock_for(&d, ACTIVE), fallback(&d)), want, "{d:?}");
            }
        }
    }

    #[test]
    fn a_seat_that_ran_out_of_time_blocks_only_where_a_rule_requires_it() {
        use mtg_engine::decision::BlockerOption;
        let kind = DecisionKind::DeclareBlockers(BlockOptions {
            blockers: vec![
                BlockerOption {
                    object: ObjectId(1),
                    can_block: vec![ObjectId(9)],
                },
                BlockerOption {
                    object: ObjectId(2),
                    can_block: vec![ObjectId(9)],
                },
            ],
            min_blockers: Vec::new(),
            required: vec![ObjectId(2)],
        });
        assert_eq!(
            fallback(&decision(OTHER, kind)),
            Some(Answer::DeclareBlockers(vec![(ObjectId(2), ObjectId(9))]))
        );
    }

    #[test]
    fn the_mode_fallback_is_a_choice_the_options_allow_whenever_it_gives_one() {
        for (count, min, max, repeat) in mode_shapes() {
            let kind = modes(count, min, max, repeat, false);
            let DecisionKind::ChooseMode(options) = &kind else {
                unreachable!("modes() builds a mode decision");
            };
            let wanted = usize::try_from(min.max(1)).unwrap();
            let feasible = count > 0 && (count >= wanted || repeat);
            let Some(Answer::ChooseMode(ModeChoice::Modes(chosen))) =
                fallback(&decision(OTHER, kind.clone()))
            else {
                assert!(!feasible, "{kind:?} has an answer to give");
                continue;
            };
            assert!(feasible, "{kind:?} gave an answer it cannot");
            let offered: Vec<usize> = options.modes.iter().map(|m| m.index).collect();
            assert!(chosen.iter().all(|i| offered.contains(i)), "{chosen:?}");
            assert_eq!(chosen.len(), wanted, "as many as the choice has to have");
            assert!(
                chosen.len() <= max as usize,
                "{chosen:?} is more than the most"
            );
            let mut once = chosen.clone();
            once.dedup();
            assert!(repeat || once.len() == chosen.len(), "{chosen:?} repeats");
        }
    }

    #[test]
    fn the_card_fallback_is_the_first_cards_offered_in_every_shape() {
        for count in 0..=6 {
            for min in 0..=4 {
                for max in min..=5 {
                    let kind = cards(count, min, max);
                    let Some(Answer::ChooseCards(CardsChoice(picked))) =
                        fallback(&decision(OTHER, kind.clone()))
                    else {
                        panic!("{kind:?} has an answer to give");
                    };
                    let wanted = usize::try_from(min).unwrap().min(count);
                    assert_eq!(picked, offered(count)[..wanted], "{kind:?}");
                    assert!(
                        picked.len() <= max as usize,
                        "{kind:?} gave more than the most"
                    );
                }
            }
        }
    }

    /// The question of the legend rule (CR 704.5j) is a choice of one among the copies, asked outside any resolution: the
    /// clock answers it like any choice of cards, with the first copy offered, and the engine takes the answer.
    #[test]
    fn the_clock_keeps_the_oldest_copy_when_the_legend_rule_asks() {
        use mtg_engine::zones::Zone;
        use mtg_testkit::attach_cards::legendary_bear;
        use mtg_testkit::{Driver, Scenario, ready};

        let mut driver = Driver::from_scenario(Scenario::new(3).battlefield(
            1,
            [
                ready(legendary_bear()),
                ready(legendary_bear()),
                ready(legendary_bear()),
            ],
        ))
        .unwrap();
        let asked = driver.decision().unwrap().clone();
        assert_eq!(asked.player, PlayerId(1));
        let DecisionKind::ChooseCards(options) = &asked.kind else {
            panic!("the legend rule asks for cards: {asked:?}");
        };
        assert_eq!((options.min, options.max, options.from.len()), (1, 1, 3));
        let answer = fallback(&asked).expect("a choice of cards has a fallback");
        assert_eq!(answer, chosen(&[options.from[0].get()]));
        driver.answer(answer).unwrap();
        let kept: Vec<ObjectId> = driver.state().zone(Zone::Battlefield).to_vec();
        assert_eq!(kept, [options.from[0]]);
        assert!(matches!(
            driver.decision().unwrap().kind,
            DecisionKind::Priority(_)
        ));
    }

    #[test]
    fn the_order_fallback_puts_every_card_once_in_the_order_given() {
        for count in 0..=7 {
            assert_eq!(
                fallback(&decision(OTHER, ordering(count))),
                Some(Answer::OrderCards(OrderChoice(offered(count))))
            );
        }
    }

    /// Every (modes on offer, fewest, most, repeat) of a small range, `min <= max`, `max >= 1`.
    fn mode_shapes() -> Vec<(usize, u32, u32, bool)> {
        let mut all = Vec::new();
        for count in 0..=5 {
            for min in 0..=3 {
                for max in min.max(1)..=4 {
                    for repeat in [false, true] {
                        all.push((count, min, max, repeat));
                    }
                }
            }
        }
        all
    }

    #[test]
    fn every_decision_that_takes_thought_has_an_answer_for_the_seat_that_ran_out_of_time() {
        for kind in think_kinds() {
            let d = decision(OTHER, kind);
            assert!(fallback(&d).is_some(), "{d:?}");
        }
    }

    #[test]
    fn the_clock_and_the_fallback_of_what_takes_thought_are_the_same_for_every_seat() {
        for kind in think_kinds() {
            let want = (
                pace().clock_for(&decision(OTHER, kind.clone()), ACTIVE),
                fallback(&decision(OTHER, kind.clone())),
            );
            for seat in 1..12 {
                let d = decision(PlayerId::from_index(seat), kind.clone());
                assert_eq!((pace().clock_for(&d, ACTIVE), fallback(&d)), want, "{d:?}");
            }
        }
    }

    #[rstest]
    #[case::auto_pass_on(true, FullControlMode::Off)]
    #[case::auto_pass_off(false, FullControlMode::Held)]
    fn a_seat_starts_in_the_mode_the_auto_pass_default_names(
        #[case] auto_pass: bool,
        #[case] mode: FullControlMode,
    ) {
        assert_eq!(
            Pace {
                auto_pass,
                ..pace()
            }
            .mode(),
            mode
        );
    }

    /// One decision of every kind with the class its clock belongs to: the table a new kind has to be added to.
    fn every_kind() -> Vec<(DecisionKind, Class)> {
        vec![
            (mulligan(), Class::Opening),
            (bottoming(7, 1), Class::Opening),
            (priority(true), Class::Response),
            (priority(false), Class::Response),
            (targets(true), Class::Response),
            (order(2), Class::Response),
            (replacements(2), Class::Response),
            (question(), Class::Response),
            (options(3), Class::Response),
            (modes(3, 1, 1, false, true), Class::Response),
            (number(0, Some(3), true), Class::Response),
            (division(3, 2, true), Class::Response),
            (costs(2, true), Class::Response),
            (cards(3, 1, 1), Class::Response),
            (ordering(3), Class::Response),
            (blockers(), Class::Blocks),
            (payment(true, true), Class::Think),
            (damage(3, &[2, 2], false), Class::Think),
            (attackers(&[]), Class::Think),
            (discard(8, 1), Class::Think),
        ]
    }

    #[test]
    fn every_kind_has_a_class_and_the_table_names_each_kind() {
        let names: std::collections::BTreeSet<_> =
            every_kind().iter().map(|(kind, _)| kind.name()).collect();
        assert_eq!(names.len(), 19, "one row for each decision kind: {names:?}");
        for (kind, class) in every_kind() {
            assert_eq!(Class::of(&kind), class, "{}", kind.name());
        }
    }

    #[rstest]
    #[case::no_active_clock(None)]
    #[case::thirty_seconds(Some(30))]
    fn the_active_seat_runs_on_the_active_length_for_every_kind_but_the_opening(
        #[case] active: Option<u64>,
    ) {
        let settings = Pace {
            active: active.map(Duration::from_secs),
            ..pace()
        };
        for (kind, class) in every_kind() {
            let got = settings.clock_for(&decision(ACTIVE, kind.clone()), ACTIVE);
            let expected = match class {
                Class::Opening => settings.think,
                _ => settings.active,
            };
            assert_eq!(got, expected, "{}", kind.name());
        }
    }

    #[test]
    fn a_seat_that_is_not_the_active_player_never_runs_on_the_active_length() {
        let settings = Pace {
            active: Some(Duration::from_secs(30)),
            ..pace()
        };
        for (kind, class) in every_kind() {
            let expected = match class {
                Class::Opening | Class::Think => settings.think,
                Class::Response => settings.response,
                Class::Blocks => settings.blocks,
            };
            for player in [OTHER, PlayerId(2), PlayerId(3)] {
                let got = settings.clock_for(&decision(player, kind.clone()), ACTIVE);
                assert_eq!(got, expected, "{}", kind.name());
            }
        }
    }

    #[test]
    fn the_answer_at_the_deadline_is_the_same_for_the_active_seat_and_every_other() {
        for (kind, _) in every_kind() {
            let (active, other) = (
                fallback(&decision(ACTIVE, kind.clone())),
                fallback(&decision(OTHER, kind.clone())),
            );
            assert_eq!(active, other, "{}", kind.name());
        }
    }

    #[rstest]
    #[case::no_grace_shows_at_once(0, 10, 0)]
    #[case::three_seconds_of_ten(3, 10, 3)]
    #[case::three_seconds_of_forty_five(3, 45, 3)]
    #[case::three_seconds_of_sixty(3, 60, 3)]
    #[case::a_clock_just_longer_than_the_grace(3, 4, 3)]
    #[case::a_clock_as_long_as_the_grace_shows_at_once(3, 3, 0)]
    #[case::a_clock_shorter_than_the_grace_shows_at_once(30, 10, 0)]
    fn the_countdown_is_delayed_by_the_grace_when_the_clock_is_longer(
        #[case] grace: u64,
        #[case] length: u64,
        #[case] shown_after: u64,
    ) {
        let settings = Pace {
            grace: Duration::from_secs(grace),
            ..pace()
        };
        assert_eq!(
            settings.shown_after(Duration::from_secs(length)),
            Duration::from_secs(shown_after)
        );
    }

    #[test]
    fn the_delay_follows_the_clock_of_every_kind_and_seat_and_nothing_else() {
        let settings = Pace {
            active: Some(Duration::from_secs(30)),
            ..pace()
        };
        for (kind, _) in every_kind() {
            for player in [ACTIVE, OTHER, PlayerId(2), PlayerId(3)] {
                let d = decision(player, kind.clone());
                let length = settings.clock_for(&d, ACTIVE).expect("a clock");
                assert_eq!(
                    settings.shown_after(length),
                    Duration::from_secs(3),
                    "{}",
                    kind.name()
                );
            }
        }
    }

    #[test]
    fn the_priority_clock_and_its_delay_ignore_whether_the_seat_can_act() {
        let settings = pace();
        let shown = |can_act| {
            let d = decision(OTHER, priority(can_act));
            settings
                .clock_for(&d, ACTIVE)
                .map(|length| (length, settings.shown_after(length)))
        };
        assert_eq!(shown(true), shown(false));
        assert_eq!(
            shown(true),
            Some((Duration::from_secs(10), Duration::from_secs(3)))
        );
    }

    #[test]
    fn only_priority_is_a_priority_decision() {
        assert!(is_priority(&decision(OTHER, priority(true))));
        assert!(is_priority(&decision(OTHER, priority(false))));
        assert!(!is_priority(&decision(OTHER, blockers())));
        for kind in think_kinds() {
            assert!(!is_priority(&decision(OTHER, kind)));
        }
    }
}
