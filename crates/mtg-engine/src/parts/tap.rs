//! Tapping untapped permanents the payer controls as a cost ("Tap an untapped creature you control"). Summoning
//! sickness does not matter: it holds back the `{T}` of the creature itself, not the tapping of another (CR 302.6).

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::characteristics::controller_of;
use crate::events::Event;
use crate::zones::Zone;

/// The untapped permanents the payer controls that match.
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
                    .is_some_and(|permanent| !permanent.tapped())
        },
    )
}

/// The permanent becomes tapped.
pub(super) const fn event(object: ObjectId) -> Event {
    Event::PermanentTapped { object }
}

/// "Tap an untapped creature you control.", "Tap 2 untapped creatures you control."
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::tap_untapped(filter, count)
}
