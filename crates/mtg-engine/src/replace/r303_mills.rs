//! "As this creature enters, mill three cards for each time it was kicked." (`ReplaceBy::Mill`): the controller mills
//! as the permanent enters (CR 701.17), each milled card a move of its own after the enter event, so a replacement of a
//! card's move (Rest in Peace) still applies to it.

use mtg_cards::value::Amount;
use mtg_core::ids::PlayerId;

use super::{Candidate, Item, Pass, Slot};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::resolve::Scope;
use crate::zones::Zone;

impl Pass<'_> {
    /// Mills `count` cards of `controller`'s library (the amount read for the entering card) into the batch after the
    /// enter event at `at`; nothing for a player who has left or an amount of nothing.
    pub(super) fn r303_mills(
        &mut self,
        at: usize,
        candidate: &Candidate,
        controller: PlayerId,
        count: &Amount,
    ) -> Result<(), EngineError> {
        if !self.state.is_alive(controller) {
            return Ok(());
        }
        let scope = Scope::announcing(self.state, candidate.controller, candidate.object);
        let wanted = usize::try_from(scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
        let milled = top_cards(self.state, controller, wanted);
        for (offset, object) in milled.into_iter().enumerate() {
            self.slots.insert(
                at + 1 + offset,
                Slot::new(Item::One(Event::ZoneChanged {
                    object,
                    from: Zone::Library(controller),
                    to: Zone::Graveyard(controller),
                    cause: ZoneCause::Milled,
                })),
            );
        }
        Ok(())
    }
}
