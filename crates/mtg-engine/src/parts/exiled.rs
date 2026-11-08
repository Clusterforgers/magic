//! Putting exiled cards into their owners' graveyards as a cost ("Put two cards your opponents own from exile into their
//! owners' graveyards").

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The exiled cards that match, whoever controls the effect that exiled them.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Exile).iter().copied(),
        filter,
        |_| true,
    )
}

/// The card leaves exile for its owner's graveyard.
pub(super) fn event(state: &GameState, object: ObjectId) -> Option<Event> {
    Some(Event::ZoneChanged {
        object,
        from: Zone::Exile,
        to: Zone::Graveyard(state.object(object)?.owner()),
        cause: ZoneCause::Moved,
    })
}

/// "Put a card from exile into its owner's graveyard.", "Put 2 cards from exile into their owners' graveyards."
pub(super) fn text(count: usize) -> String {
    if count == 1 {
        "Put a card from exile into its owner's graveyard.".to_owned()
    } else {
        format!("Put {count} cards from exile into their owners' graveyards.")
    }
}
