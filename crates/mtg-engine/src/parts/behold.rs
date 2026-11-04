//! Beholding as a cost: the payer chooses a permanent they control that matches, or reveals a card from their hand that
//! matches. Only the revealed card is shown; a permanent chosen stays as it is.

use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use super::Payer;
use crate::characteristics::controller_of;
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// The permanents the payer controls and the cards in their hand that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    let mut found = payer.matching(
        payer.state.zone(Zone::Battlefield).iter().copied(),
        filter,
        |id| controller_of(payer.state, id) == Some(payer.player),
    );
    found.extend(payer.matching(
        payer.state.zone(Zone::Hand(payer.player)).iter().copied(),
        filter,
        |_| true,
    ));
    found
}

/// "Behold a Goat.": the choice is a permanent or a card of the hand.
pub(super) fn text(filter: &Selector, count: usize) -> String {
    let revealed = crate::wording::reveal_from_hand(filter, count);
    format!(
        "Behold: choose {} you control or {}",
        crate::wording::subject(filter),
        revealed.to_lowercase()
    )
}

/// A card in the hand is revealed; a permanent is only chosen, so nothing happens to it.
pub(super) fn event(state: &GameState, player: PlayerId, object: ObjectId) -> Option<Event> {
    state
        .zone(Zone::Hand(player))
        .contains(&object)
        .then(|| super::reveal::event(player, object))
}
