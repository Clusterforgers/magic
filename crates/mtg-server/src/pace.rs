//! Pace of play: passing for a seat the windows pass for or that yields, and the clocks that keep one player from
//! holding the table.
//!
//! All of them work through the same path as a seat's own answer ([`crate::running::Running::answer`]), so what a
//! pass or a timeout does to the log, the store and every projection is exactly what the seat's click would have done.
//! When a seat is asked is decided in [`crate::windows`] and when a yield stops in [`crate::yields`], alone. A seat that
//! cannot act at a window where it waits is passed for the same way once its dwell ([`crate::dwell`]) is over.

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{
    CardsChoice, CardsOptions, CostsChoice, DivideChoice, DivideOptions, ModeChoice, ModeOptions,
    NumberChoice, OptionChoice, OrderChoice, ReplacementChoice, Target, TargetChoice,
    TriggersChoice, YesNoChoice,
};
use mtg_engine::decision::PriorityAction;
use mtg_engine::{Answer, Decision, DecisionKind};
use mtg_protocol::{ErrorCode, FullControlMode, ServerMessage, YieldUntil};
use tokio::time::Instant;

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::dwell::Dwell;
use crate::running::{Halt, Rejection};
use crate::store::EventStore;
use crate::windows::Verdict;
use crate::yields::Yields;

/// Longest clock the settings accept, in seconds (a day), so a deadline can never overflow.
pub const MAX_CLOCK_SECS: u64 = 86_400;

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
    /// How long the table waits before it passes for an idle seat at a window of the default table, its own main phase
    /// with an empty stack aside (`MTG_DWELL_MS`).
    pub dwell: Dwell,
}

impl Pace {
    /// No auto-pass and no clocks: every decision waits for its player, as in a game played straight on an engine.
    pub const MANUAL: Pace = Pace {
        auto_pass: false,
        response: None,
        blocks: None,
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

    /// The clock `decision` runs on, if any.
    ///
    /// Only a seat that is not the active player is hurried, and only for a priority decision, for choosing the
    /// targets of a spell or ability, for ordering its triggers or the replacement effects that apply to an event, for
    /// answering a question of an effect (yes or no, an option, modes, a number, optional costs, a division, cards, an order of cards) or for declaring blockers; every other decision (mulligans, discards,
    /// damage division, the active player's own turn) waits for as long as it takes. A priority window the seat has nothing to do in is
    /// hurried too (it is only delivered at a stop or in Full Control): it would hold the table for no reason. The
    /// rule reads the kind alone, never the options, the seat or the hand, so it shows nothing about them.
    pub(crate) fn clock_for(&self, decision: &Decision, active: PlayerId) -> Option<Duration> {
        if decision.player == active {
            return None;
        }
        match &decision.kind {
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
            | DecisionKind::OrderCards(_) => self.response,
            DecisionKind::DeclareBlockers(_) => self.blocks,
            _ => None,
        }
    }
}

impl Default for Pace {
    fn default() -> Self {
        Pace {
            auto_pass: true,
            response: Some(Duration::from_secs(10)),
            blocks: Some(Duration::from_secs(45)),
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

/// What the table answers for a seat whose clock ran out: a pass, only the blockers a rule requires, no target (a spell is taken back and a
/// triggered ability removed, as the rules do when a target cannot be chosen), the triggers in the order they
/// triggered, No to a question, the first option, for modes the cast taken back when the options allow it and the
/// first mode otherwise, the same for a number, the optional costs and a division (the lowest number, no cost and an
/// even division otherwise), the first cards of the list for a choice of cards and the order given for an order of cards.
///
/// The answer is read from the decision alone. `None` for a decision that has no clock and no safe default.
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
        DecisionKind::ChooseCards(options) => Some(Answer::ChooseCards(first_cards(options))),
        DecisionKind::OrderCards(options) => {
            Some(Answer::OrderCards(OrderChoice(options.cards.clone())))
        }
        _ => None,
    }
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

    /// Answers for the seat whose clock ran out, with a pass or no blocks, as if the seat had. The same step passes for a
    /// seat that cannot act once its dwell is over: the decision was never sent, and the pass is saved, projected and
    /// sent like the seat's own, as a batch of its own.
    pub(crate) async fn expire(&mut self) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            return Ok(());
        };
        if let Ok(step) = running.fall_back() {
            // If the move cannot be saved the decision stays; trying again at once would spin.
            running.postpone(Instant::now() + RETRY_AFTER);
            self.publish(step).await
        } else {
            running.stop_clock();
            Ok(())
        }
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
    use mtg_engine::decision::{
        AttackOptions, BlockOptions, BottomOptions, DamageOptions, DiscardOptions, MulliganOptions,
        PayOptions, PriorityOptions,
    };
    use mtg_engine::mana::ManaPool;
    use mtg_engine::object::ObjectRef;
    use rstest::rstest;

    const ACTIVE: PlayerId = PlayerId(0);
    const OTHER: PlayerId = PlayerId(1);

    fn priority(can_act: bool) -> DecisionKind {
        DecisionKind::Priority(PriorityOptions {
            playable_lands: Vec::new(),
            castable: Vec::new(),
            mana_abilities: Vec::new(),
            abilities: Vec::new(),
            can_act,
            can_pass: true,
        })
    }

    fn blockers() -> DecisionKind {
        DecisionKind::DeclareBlockers(BlockOptions {
            blockers: Vec::new(),
            min_blockers: Vec::new(),
            required: Vec::new(),
        })
    }

    fn other_kinds() -> Vec<DecisionKind> {
        vec![
            DecisionKind::Mulligan(MulliganOptions {
                hand: Vec::new(),
                mulligans_taken: 0,
                can_mulligan: true,
                free_mulligans: 0,
                bottom_if_keep: 0,
                bottom_if_mulligan: 1,
            }),
            DecisionKind::BottomCards(BottomOptions {
                hand: Vec::new(),
                count: 1,
            }),
            DecisionKind::PayMana(PayOptions {
                spell: ObjectId(1),
                cost: mtg_core::mana::ManaCost::parse("{1}").unwrap(),
                pool: ManaPool::default(),
                sources: Vec::new(),
                auto_available: true,
                can_cancel: true,
                life: 20,
                max_life: 0,
            }),
            DecisionKind::DeclareAttackers(AttackOptions {
                attackers: Vec::new(),
                required: Vec::new(),
            }),
            DecisionKind::AssignDamage(DamageOptions {
                attacker: ObjectId(1),
                power: 2,
                blockers: Vec::new(),
                excess: None,
                lethal: Vec::new(),
            }),
            DecisionKind::Discard(DiscardOptions {
                hand: Vec::new(),
                count: 1,
            }),
        ]
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
        DecisionKind::YesNo(YesNoOptions {
            source: ObjectId(1),
            text: "Pay {1}?".into(),
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
        })
    }

    fn ordering(count: usize) -> DecisionKind {
        DecisionKind::OrderCards(OrderOptions {
            source: ObjectId(1),
            cards: offered(count),
            text: String::new(),
        })
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
    fn clocks_by_decision(
        #[case] player: PlayerId,
        #[case] kind: DecisionKind,
        #[case] seconds: Option<u64>,
    ) {
        let got = pace().clock_for(&decision(player, kind), ACTIVE);
        assert_eq!(got, seconds.map(Duration::from_secs));
    }

    #[test]
    fn every_other_decision_waits_as_long_as_it_takes() {
        for kind in other_kinds() {
            for player in [ACTIVE, OTHER] {
                let d = decision(player, kind.clone());
                assert_eq!(pace().clock_for(&d, ACTIVE), None, "{d:?}");
            }
        }
    }

    #[rstest]
    #[case::manual(Pace::MANUAL)]
    #[case::response_off(Pace { response: None, ..pace() })]
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
    }

    #[test]
    fn the_defaults_are_ten_and_forty_five_seconds_and_a_dwell_with_auto_pass_on() {
        let pace = Pace::default();
        assert!(pace.auto_pass);
        assert_eq!(pace.response, Some(Duration::from_secs(10)));
        assert_eq!(pace.blocks, Some(Duration::from_secs(45)));
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
    fn safe_defaults(#[case] kind: DecisionKind, #[case] expected: Option<Answer>) {
        assert_eq!(fallback(&decision(OTHER, kind)), expected);
    }

    #[test]
    fn the_clock_and_the_fallback_are_the_same_for_every_seat_and_every_shape_of_options() {
        let kinds = [
            question(),
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
    fn nothing_else_has_a_safe_default() {
        for kind in other_kinds() {
            assert_eq!(fallback(&decision(OTHER, kind)), None);
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

    #[test]
    fn only_priority_is_a_priority_decision() {
        assert!(is_priority(&decision(OTHER, priority(true))));
        assert!(is_priority(&decision(OTHER, priority(false))));
        assert!(!is_priority(&decision(OTHER, blockers())));
        for kind in other_kinds() {
            assert!(!is_priority(&decision(OTHER, kind)));
        }
    }
}
