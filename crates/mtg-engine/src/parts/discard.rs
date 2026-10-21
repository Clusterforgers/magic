//! Discarding cards from the payer's hand as a cost (CR 701.9).

use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use super::Payer;
use crate::events::Event;
use crate::zones::Zone;

/// The cards in the payer's hand that match; any card when the filter is empty.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Hand(payer.player)).iter().copied(),
        filter,
        |_| true,
    )
}

/// The card is discarded: public, whoever sees the hand.
pub(super) const fn event(player: PlayerId, object: ObjectId) -> Event {
    Event::CardDiscarded { player, object }
}
