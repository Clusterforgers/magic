//! Untapping tapped permanents the payer controls as a cost ("Untap two tapped creatures you control").

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::characteristics::controller_of;
use crate::events::Event;
use crate::zones::Zone;

/// The tapped permanents the payer controls that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Battlefield).iter().copied(),
        filter,
        |id| {
            controller_of(payer.state, id) == Some(payer.player)
                && payer
                    .state
                    .object(id)
                    .and_then(|object| object.permanent())
                    .is_some_and(crate::object::PermanentState::tapped)
        },
    )
}

/// The permanent becomes untapped.
pub(super) const fn event(object: ObjectId) -> Event {
    Event::PermanentUntapped { object }
}

/// "Untap a tapped creature you control", "Untap 2 tapped creatures you control".
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::tap_untapped(filter, count)
        .replacen("Tap", "Untap", 1)
        .replacen("untapped", "tapped", 1)
}
