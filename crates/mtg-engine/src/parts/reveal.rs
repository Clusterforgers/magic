//! Revealing cards from the payer's hand as a cost ("Reveal a Giant card from your hand"): they stay in the hand.

use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use super::Payer;
use crate::events::Event;
use crate::library::Revealed;
use crate::zones::Zone;

/// The cards in the payer's hand that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Hand(payer.player)).iter().copied(),
        filter,
        |_| true,
    )
}

/// The card is shown to every player; it does not move.
pub(super) fn event(player: PlayerId, object: ObjectId) -> Event {
    Event::Revealed(Revealed {
        player,
        cards: vec![object],
    })
}

/// "Reveal a Giant card from your hand.", "Reveal 2 cards from your hand."
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::reveal_from_hand(filter, count)
}
