//! Entering under a chosen opponent's control (`ReplaceBy::UnderChosenOpponent`) in the replacement step: the controller
//! picks an opponent (nobody is asked when there is one) and the `Entered` event names that player as its controller.

use mtg_cards::trigger::EntersChoice;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Asked, Item, Pass};
use crate::error::EngineError;
use crate::events::Event;
use crate::object::Chosen;

impl Pass<'_> {
    /// Asks `controller` for an opponent and has `object` enter under that opponent's control.
    pub(super) fn ep28_under(
        &mut self,
        at: usize,
        source: ObjectId,
        object: ObjectId,
        controller: PlayerId,
    ) -> Result<Asked<()>, EngineError> {
        let chosen = match self.chooses(source, controller, EntersChoice::Opponent)? {
            Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
            Asked::Got(chosen) => chosen,
        };
        if let Some(Chosen::Player(opponent)) = chosen {
            let entered = self.entered_at(at, object, controller);
            if let Item::One(Event::Entered {
                controller: under, ..
            }) = &mut self.slots[entered].item
            {
                *under = opponent;
            }
        }
        Ok(Asked::Got(()))
    }
}
