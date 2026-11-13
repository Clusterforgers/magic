//! A choice made as an attachment becomes attached ("As this Equipment becomes attached to a creature, choose a color.",
//! `ReplaceWhen::BecomesAttached`): the controller of the attachment chooses as the `Attached` event is replaced, and a
//! `ChosenAsAttached` in front of it keeps the choice on the attachment, in place of any earlier one.

use mtg_cards::trigger::EntersChoice;
use mtg_core::ids::ObjectId;

use super::{Applied, Asked, Candidate, Item, Origin, Pass, Slot};
use crate::error::EngineError;
use crate::events::Event;

impl Pass<'_> {
    /// Asks the controller of `candidate` for the choice of `kind` and puts what they chose on the attachment `object`
    /// before its attaching at `at`.
    pub(super) fn r5501_choose_as_attached(
        &mut self,
        at: usize,
        candidate: &Candidate,
        kind: EntersChoice,
        object: ObjectId,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let chosen = match self.chooses(candidate.object.id, candidate.controller, kind)? {
            Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
            Asked::Got(chosen) => chosen,
        };
        self.slots[at].applied = applied;
        if let Some(chosen) = chosen {
            self.slots.insert(
                at,
                Slot {
                    item: Item::One(Event::ChosenAsAttached { object, chosen }),
                    applied: Vec::new(),
                    frozen: true,
                },
            );
        }
        Ok(Applied::Done)
    }
}
