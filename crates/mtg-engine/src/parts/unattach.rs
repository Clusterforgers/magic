//! Unattaching the source as a cost (CR 701.3d): an Equipment or Aura attached to a permanent or a player lets go. In
//! an ability that an Equipment or Aura gives the permanent it is attached to, that Equipment or Aura lets go ("Equipped
//! creature has '{T}, Unattach Shuriken: ...'").

use mtg_core::ids::ObjectId;

use super::Payer;
use crate::events::Event;

/// Whether the source is attached to something, the only thing that can let go; for a given ability, whether the
/// permanent that gives it is attached to the source.
pub(super) fn payable(payer: &Payer<'_>) -> bool {
    if let Some(granter) = payer.granter {
        return payer
            .state
            .object(granter)
            .and_then(|object| object.permanent())
            .and_then(|permanent| permanent.attached_to())
            .is_some_and(|host| host.id == payer.source);
    }
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
