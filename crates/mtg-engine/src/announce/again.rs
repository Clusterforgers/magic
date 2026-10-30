//! An announcement a manual action came in the middle of is asked again from its start against the new state: each
//! answer given so far is given again while it is still legal, and the first that no longer is (a pick tapped by hand,
//! a target gone, a card it chose moved) leaves its step asked anew with the candidates of now, instead of a later
//! answer or the payment being refused for good.

use crate::answers::{Outcome, handle};
use crate::choose::Choices;
use crate::decision::Answer;
use crate::state::GameState;

use super::{Answered, Done, Next, Pending, announce};

impl Answered {
    /// Whether `outcome` answers a question of the announcement.
    pub(crate) const fn of_kind(outcome: &Outcome) -> bool {
        matches!(
            outcome,
            Outcome::Modes(_)
                | Outcome::Way(_)
                | Outcome::Targets(_)
                | Outcome::Number(_)
                | Outcome::Costs(_)
                | Outcome::Cards(_)
                | Outcome::Division(_)
        )
    }

    /// The answer to a question of the announcement that `outcome` is, if it is one.
    pub(crate) fn of(outcome: Outcome) -> Option<Answered> {
        Some(match outcome {
            Outcome::Modes(modes) => Answered::Modes(modes),
            Outcome::Way(pick) => Answered::Way(pick),
            Outcome::Targets(targets) => Answered::Targets(targets),
            Outcome::Number(number) => Answered::Number(number),
            Outcome::Costs(costs) => Answered::Costs(costs),
            Outcome::Cards(cards) => Answered::Cards(cards),
            Outcome::Division(shares) => Answered::Division(shares),
            _ => return None,
        })
    }
}

impl Pending {
    /// Notes `answer`, which the announcement took, so it can be given again (`again`).
    pub(crate) fn note(&mut self, answer: Answer) {
        self.answers.push(answer);
    }

    /// Marks the announcement to be asked again before its next question: a manual action changed the state under it.
    pub(crate) fn unsettle(&mut self) {
        self.stale = true;
    }

    /// Asks the announcement again if a manual action unsettled it: the answers it took are given again in order
    /// against `state` until one is no longer legal, whose step is then the next question. The anchor stays the one it
    /// began with, so a card or source moved away still ends it (`anchor.rs`).
    pub(crate) fn settle(&mut self, state: &GameState) {
        if !std::mem::take(&mut self.stale) {
            return;
        }
        let answers = std::mem::take(&mut self.answers);
        self.choices = Choices::default();
        self.done = Done::default();
        self.needs = None;
        for answer in answers {
            let Next::Ask(decision) = announce(state, self) else {
                return;
            };
            let Some(answered) = handle(state, Some(self), false, &decision, answer.clone())
                .ok()
                .and_then(Answered::of)
            else {
                return;
            };
            if self.accept(answered).is_err() {
                return;
            }
            self.answers.push(answer);
        }
    }
}
