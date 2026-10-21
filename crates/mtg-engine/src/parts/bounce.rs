//! Returning a permanent the payer controls to its owner's hand as a cost.

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::characteristics::controller_of;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The permanents the payer controls that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Battlefield).iter().copied(),
        filter,
        |id| controller_of(payer.state, id) == Some(payer.player),
    )
}

/// The permanent goes to its owner's hand.
pub(super) fn event(state: &GameState, object: ObjectId) -> Option<Event> {
    Some(Event::ZoneChanged {
        object,
        from: Zone::Battlefield,
        to: Zone::Hand(state.object(object)?.owner()),
        cause: ZoneCause::Moved,
    })
}

/// "Return a land you control to its owner's hand."
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::return_to_hand(filter, count)
}
