//! Validating an answer against the decision it responds to, without touching state.

use mtg_core::ids::ObjectId;

use crate::combat::{attack_events, check_attackers, check_blockers};
use crate::damage::assignment_answer;
use crate::decision::{Answer, Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::mulligan::{bottom_answer, check_card_choice, mulligan_answer};
use crate::{cast, priority};

/// What a validated answer asks the engine to do.
#[derive(Debug)]
pub(crate) enum Outcome {
    /// Fold these events.
    Events(Vec<Event>),
    /// Start casting this spell; payment is decided next.
    BeginCast(ObjectId),
    /// Abandon the cast in progress.
    CancelCast,
}

/// Checks `answer` against `decision`; the result is only built from legal options.
pub(crate) fn handle(decision: &Decision, answer: Answer) -> Result<Outcome, EngineError> {
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
            cast::pay_answer(player, options, choice)
        }
        (DecisionKind::DeclareAttackers(options), Answer::DeclareAttackers(attackers)) => {
            check_attackers(options, &attackers)?;
            Ok(Outcome::Events(attack_events(player, &attackers)))
        }
        (DecisionKind::DeclareBlockers(options), Answer::DeclareBlockers(blocks)) => {
            check_blockers(options, &blocks)?;
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
        (kind, answer) => Err(IllegalAnswer::WrongKind {
            expected: kind.name(),
            got: answer.name(),
        }
        .into()),
    }
}
