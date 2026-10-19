//! Validating an answer against the decision it responds to, without touching state.

use mtg_core::ids::ObjectId;

use crate::abilities::AbilityRef;
use crate::choose::{self, Choices, Target};
use crate::combat::{attack_events, check_attackers, check_blockers};
use crate::damage::assignment_answer;
use crate::decision::{Answer, Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::mulligan::{bottom_answer, check_card_choice, mulligan_answer};
use crate::state::GameState;
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
    /// These are the targets of the next specification of the cast or activation in progress.
    Targets(Vec<Target>),
    /// Abandon the cast or activation in progress.
    CancelCast,
}

/// Checks `answer` against `decision`; the result is only built from legal options.
///
/// `announced` is what the cast in progress has announced so far, which its payment puts on the spell.
pub(crate) fn handle(
    state: &GameState,
    announced: &Choices,
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
            priority::answer(player, options, action)
        }
        (DecisionKind::PayMana(options), Answer::PayMana(choice)) => {
            cast::pay_answer(state, player, options, choice, announced)
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
            choose::targets_answer(player, options, choice)
        }
        (DecisionKind::ChooseMode(options), Answer::ChooseMode(choice)) => {
            choose::mode_answer(player, options, choice)
        }
        (DecisionKind::ChooseNumber(options), Answer::ChooseNumber(choice)) => {
            choose::number_answer(player, options, choice)
        }
        (DecisionKind::DivideAmount(options), Answer::DivideAmount(choice)) => {
            choose::divide_answer(player, options, choice)
        }
        (DecisionKind::ChooseCards(options), Answer::ChooseCards(choice)) => {
            choose::cards_answer(player, options, choice)
        }
        (DecisionKind::OrderCards(options), Answer::OrderCards(choice)) => {
            choose::order_answer(player, options, choice)
        }
        (DecisionKind::YesNo(options), Answer::YesNo(choice)) => {
            choose::yes_no_answer(player, options, choice)
        }
        (DecisionKind::ChooseOption(options), Answer::ChooseOption(choice)) => {
            choose::option_answer(player, options, choice)
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
