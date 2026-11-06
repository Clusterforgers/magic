//! Tribute N (CR 702.104) in the replacement step: an opponent the controller picks may put N +1/+1 counters on the
//! permanent as it enters, and the permanent remembers whether they did.

use mtg_cards::trigger::EntersChoice;
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Asked, Candidate, Origin, Pass};
use crate::choose::{Choice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::object::Chosen;

impl Pass<'_> {
    /// Asks the controller for an opponent (nobody is asked when there is one or none), then that opponent whether to pay
    /// tribute. A paid tribute gives the counters; either way the permanent remembers 1 or 0 for "if tribute wasn't paid".
    pub(super) fn tribute(
        &mut self,
        at: usize,
        candidate: &Candidate,
        object: ObjectId,
        controller: PlayerId,
        count: u32,
        applied: &[Origin],
    ) -> Result<Asked<()>, EngineError> {
        let source = candidate.object.id;
        let opponent = match self.chooses(source, controller, EntersChoice::Opponent)? {
            Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
            Asked::Got(Some(Chosen::Player(opponent))) => Some(opponent),
            Asked::Got(_) => None,
        };
        let paid = match opponent {
            None => false,
            Some(opponent) => match self.answer() {
                None => {
                    return Ok(Asked::Ask(Decision {
                        player: opponent,
                        kind: DecisionKind::YesNo(YesNoOptions::new(
                            source,
                            format!("Put {count} +1/+1 counters on it (tribute)?"),
                        )),
                    }));
                }
                Some((who, Choice::YesNo(yes))) if *who == opponent => *yes,
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the tribute of {opponent:?}"
                    )));
                }
            },
        };
        if paid {
            let number = Amount::Fixed(i32::try_from(count).unwrap_or(i32::MAX));
            self.enter_with_counters(
                at,
                candidate,
                object,
                &CounterKind::PlusOnePlusOne,
                &number,
                applied,
            )?;
        }
        self.w2b_remember(
            at,
            object,
            controller,
            Some(Chosen::Number(u32::from(paid))),
        );
        Ok(Asked::Got(()))
    }
}
