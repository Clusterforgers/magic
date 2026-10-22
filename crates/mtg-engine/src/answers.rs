//! Validating an answer against the decision it responds to, without touching state.

use mtg_core::ids::ObjectId;

use crate::abilities::AbilityRef;
use crate::announce::Pending;
use crate::choose::{self, Target};
use crate::combat::{attack_events, check_attackers, check_blockers};
use crate::damage::assignment_answer;
use crate::decision::{Answer, Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::mulligan::{bottom_answer, check_card_choice, mulligan_answer};
use crate::state::{GameState, Stage};
use crate::{cast, priority};

/// What a validated answer asks the engine to do.
#[derive(Debug)]
pub(crate) enum Outcome {
    /// Fold these events.
    Events(Vec<Event>),
    /// Start casting this spell; its announcement and payment are decided next.
    BeginCast(ObjectId),
    /// Start activating this ability; its announcement is decided next.
    BeginActivation(AbilityRef),
    /// Start suspending this card from the hand: the payment of its suspend cost is decided next.
    BeginSuspend(ObjectId),
    /// Start foretelling this card from the hand: the payment of {2} is decided next.
    BeginForetell(ObjectId),
    /// Start turning this face-down permanent face up: the payment of its cost is decided next.
    BeginTurnUp(ObjectId),
    /// These are the modes, in the order they are printed, of the cast or activation in progress.
    Modes(Vec<usize>),
    /// This is the way, by position among those offered, the spell in progress is cast.
    Way(usize),
    /// These are the targets of the next specification of the cast or activation in progress.
    Targets(Vec<Target>),
    /// This is the value of X of the cast or activation in progress.
    Number(u32),
    /// These are the optional costs, by index, of the cast or activation in progress.
    Costs(Vec<usize>),
    /// These are the permanents and cards picked for the next part of the cost of the activation in progress.
    Cards(Vec<ObjectId>),
    /// This is how the next amount to divide of the cast or activation in progress is shared among its targets.
    Division(Vec<(Target, u32)>),
    /// Abandon the cast or activation in progress.
    CancelCast,
    /// Remove the triggered ability being put on the stack, whose controller chose no target (CR 603.3d).
    DropTrigger,
}

/// Checks `answer` against `decision`; the result is only built from legal options.
///
/// `pending` is the cast or activation in progress, whose announcements its payment puts on the spell or ability;
/// `replacing` says the decision is a question of the replacement step.
pub(crate) fn handle(
    state: &GameState,
    pending: Option<&Pending>,
    replacing: bool,
    decision: &Decision,
    answer: Answer,
) -> Result<Outcome, EngineError> {
    let player = decision.player;
    match (&decision.kind, answer) {
        (DecisionKind::Mulligan(options), Answer::Mulligan(choice)) => {
            mulligan_answer(player, options, choice)
        }
        (DecisionKind::BottomCards(options), Answer::BottomCards(cards)) => {
            bottom_answer(player, options, cards)
        }
        (DecisionKind::Priority(options), Answer::Priority(action)) => {
            priority::answer(state, player, options, action)
        }
        (DecisionKind::PayMana(options), Answer::PayMana(choice)) => {
            cast::pay_answer(state, player, options, choice, pending)
        }
        (DecisionKind::DeclareAttackers(options), Answer::DeclareAttackers(attackers)) => {
            check_attackers(options, &attackers)?;
            Ok(Outcome::Events(attack_events(state, player, &attackers)))
        }
        (DecisionKind::DeclareBlockers(options), Answer::DeclareBlockers(blocks)) => {
            check_blockers(state, options, &blocks)?;
            Ok(Outcome::Events(vec![Event::BlockersDeclared {
                player,
                blocks,
            }]))
        }
        (DecisionKind::AssignDamage(options), Answer::AssignDamage(assignment)) => {
            Ok(assignment_answer(options, assignment)?)
        }
        (DecisionKind::Discard(options), Answer::Discard(cards)) => {
            check_card_choice(&options.hand, options.count, &cards)?;
            Ok(Outcome::Events(
                cards
                    .into_iter()
                    .map(|object| Event::CardDiscarded { player, object })
                    .collect(),
            ))
        }
        (DecisionKind::ChooseTargets(options), Answer::ChooseTargets(choice)) => {
            choose::targets_answer(player, options, choice, pending.is_some())
        }
        (DecisionKind::ChooseMode(options), Answer::ChooseMode(choice)) => {
            choose::mode_answer(player, options, choice)
        }
        (DecisionKind::ChooseNumber(options), Answer::ChooseNumber(choice)) => {
            choose::number_answer(player, options, choice, pending.is_some())
        }
        (DecisionKind::DivideAmount(options), Answer::DivideAmount(choice)) => {
            choose::divide_answer(player, options, choice)
        }
        // In the opening the question is the scry of a seat that kept a hand after a mulligan.
        (DecisionKind::ChooseCards(options), Answer::ChooseCards(choice))
            if state.stage() == Stage::Mulligan =>
        {
            crate::mulligan::scry_answer(player, options, &choice)
        }
        // Nothing resolving and nothing announced: the question is the legend rule of the state-based actions.
        (DecisionKind::ChooseCards(options), Answer::ChooseCards(choice))
            if state.resolving().is_none() && pending.is_none() && !replacing =>
        {
            crate::legend::answer(state, options, &choice)
        }
        (DecisionKind::ChooseCards(options), Answer::ChooseCards(choice)) => {
            let outcome = choose::cards_answer(player, options, choice, pending.is_some())?;
            if let (Outcome::Cards(cards), Some(pending)) = (&outcome, pending) {
                pending.check_picked(state, cards)?;
            }
            Ok(outcome)
        }
        (DecisionKind::OrderCards(options), Answer::OrderCards(choice)) => {
            choose::order_answer(player, options, choice)
        }
        // Nothing resolving and nothing announced: the question is the owner's choice for a commander in a graveyard or
        // exile (CR 903.9a).
        (DecisionKind::YesNo(options), Answer::YesNo(choice))
            if state.resolving().is_none()
                && pending.is_none()
                && !replacing
                && crate::commanders::asks(state, options) =>
        {
            Ok(crate::commanders::answer(state, options, choice))
        }
        (DecisionKind::YesNo(options), Answer::YesNo(choice)) => {
            choose::yes_no_answer(player, options, choice)
        }
        (DecisionKind::ChooseOption(options), Answer::ChooseOption(choice)) => {
            choose::option_answer(player, options, choice, pending.is_some())
        }
        (DecisionKind::ChooseCosts(options), Answer::ChooseCosts(choice)) => {
            choose::costs_answer(player, options, choice)
        }
        (DecisionKind::OrderTriggers(options), Answer::OrderTriggers(choice)) => {
            choose::triggers_answer(player, options, choice)
        }
        (DecisionKind::ChooseReplacement(options), Answer::ChooseReplacement(choice)) => {
            choose::replacement_answer(player, options, choice)
        }
        (kind, answer) => Err(IllegalAnswer::WrongKind {
            expected: kind.name(),
            got: answer.name(),
        }
        .into()),
    }
}
