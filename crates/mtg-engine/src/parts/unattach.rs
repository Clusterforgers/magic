//! Unattaching the source as a cost (CR 701.3d): an Equipment or Aura attached to a permanent or a player lets go.

use mtg_core::ids::ObjectId;

use super::Payer;
use crate::events::Event;

/// Whether the source is attached to something, the only thing that can let go.
pub(super) fn payable(payer: &Payer<'_>) -> bool {
    payer
        .state
        .object(payer.source)
        .and_then(|object| object.permanent())
        .is_some_and(|permanent| {
            permanent.attached_to().is_some() || permanent.attached_player().is_some()
        })
}

/// The source stops being attached.
pub(super) const fn event(source: ObjectId) -> Event {
    Event::Unattached { object: source }
}
