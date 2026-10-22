//! Exiling cards from the payer's hand as a cost ("Exile a red card from your hand").

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The cards in the payer's hand that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Hand(payer.player)).iter().copied(),
        filter,
        |_| true,
    )
}

/// The card leaves the hand for exile, face up: every seat sees it.
pub(super) fn event(state: &GameState, object: ObjectId) -> Option<Event> {
    Some(Event::ZoneChanged {
        object,
        from: state.object(object)?.zone(),
        to: Zone::Exile,
        cause: ZoneCause::Exiled,
    })
}

/// "Exile a red card from your hand.", "Exile 2 cards from your hand."
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::exile_from_hand(filter, count)
}
