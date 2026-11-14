//! An optional move into a library (`ReplaceBy::IntoLibrary { optional: true }`): "If this permanent would be put into
//! a graveyard, you may put it on top of its owner's library instead." The replacement's controller is asked as it
//! applies; a no is remembered for the event, which then goes where it was going.

use super::{Asked, Candidate, Pass};
use crate::choose::{Choice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;

impl Pass<'_> {
    /// Whether the controller of the replacement puts the object into its owner's library instead: asked yes or no.
    pub(super) fn r6008_library_wanted(
        &mut self,
        at: usize,
        candidate: &Candidate,
    ) -> Result<Asked<bool>, EngineError> {
        let player = candidate.controller;
        match self.answer() {
            None => {
                let name = self
                    .state
                    .object(candidate.object.id)
                    .map_or("the card", |card| card.card().name.as_str());
                Ok(Asked::Ask(Decision {
                    player,
                    kind: DecisionKind::YesNo(YesNoOptions::new(
                        candidate.object.id,
                        format!("Put {name} into its owner's library instead?"),
                    )),
                }))
            }
            Some((who, Choice::YesNo(yes))) if *who == player => {
                let yes = *yes;
                if !yes {
                    self.slots[at].applied.push(candidate.origin);
                }
                Ok(Asked::Got(yes))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the library question of {player:?}"
            ))),
        }
    }
}
