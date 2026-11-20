//! `ReplaceBy::R110MayPreventAmount` (CR 615.1): the replacement's controller is asked at each damage event whether to
//! prevent the amount it worked out as the event happened ("you may prevent X of that damage, where X is the number of
//! Clerics you control").

use mtg_cards::trigger::ReplaceBy;
use mtg_cards::value::Amount;

use super::{Candidate, Item, Origin, Pass};
use crate::choose::{Choice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;

impl Pass<'_> {
    /// Prevents up to `most` (fixed as the event was offered) of the damage at `at` when the controller says yes; a no
    /// leaves the damage as it is, the replacement applied to it all the same. The question comes back as the decision.
    pub(super) fn r110_may_prevent(
        &mut self,
        at: usize,
        candidate: &Candidate,
        most: &Amount,
        applied: Vec<Origin>,
    ) -> Result<Option<Decision>, EngineError> {
        let Amount::Fixed(most) = most else {
            return Err(Self::invalid(format!(
                "{most:?} is no number fixed for a prevention"
            )));
        };
        let most = u32::try_from(*most).unwrap_or(0);
        let damage = match &self.slots[at].item {
            Item::One(Event::DamageDealt { amount, .. }) => *amount,
            _ => 0,
        };
        let controller = candidate.controller;
        match self.answer() {
            None => Ok(Some(Decision {
                player: controller,
                kind: DecisionKind::YesNo(YesNoOptions::new(
                    candidate.object.id,
                    format!("Prevent {} of that damage?", most.min(damage)),
                )),
            })),
            Some((who, Choice::YesNo(true))) if *who == controller => {
                self.prevent(at, candidate, &ReplaceBy::PreventUpTo(most), applied);
                Ok(None)
            }
            Some((who, Choice::YesNo(false))) if *who == controller => {
                self.slots[at].applied = applied;
                Ok(None)
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to a prevention of {controller:?}"
            ))),
        }
    }
}
