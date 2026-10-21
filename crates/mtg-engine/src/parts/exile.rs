//! Exiling cards from the payer's graveyard as a cost ("Exile a creature card from your graveyard").

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The cards in the payer's graveyard that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer
            .state
            .zone(Zone::Graveyard(payer.player))
            .iter()
            .copied(),
        filter,
        |_| true,
    )
}

/// The card leaves its owner's graveyard for exile.
pub(super) fn event(state: &GameState, object: ObjectId) -> Option<Event> {
    Some(Event::ZoneChanged {
        object,
        from: Zone::Graveyard(state.object(object)?.owner()),
        to: Zone::Exile,
        cause: ZoneCause::Exiled,
    })
}

/// "Exile a card from your graveyard.", "Exile 2 creature cards from your graveyard."
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::exile_from_graveyard(filter, count)
}
