//! Drawing cards as a cost (CR 121): the top cards of the payer's library go to their hand.

use mtg_core::ids::PlayerId;

use super::Payer;
use crate::events::Event;
use crate::state::GameState;

/// The cards the payer could draw: the library, and nothing while they can't draw (CR 121.2).
pub(super) fn most(payer: &Payer<'_>) -> u32 {
    if crate::rules::cant_draw(payer.state, payer.player) {
        return 0;
    }
    super::mill::most(payer)
}

/// Whether the player can draw that many; a cost is not paid by a draw that fails on an empty library.
pub(super) fn payable(payer: &Payer<'_>, amount: u32) -> bool {
    amount == 0 || most(payer) >= amount
}

/// The top cards drawn, the top card first.
pub(super) fn events(state: &GameState, player: PlayerId, amount: u32) -> Vec<Event> {
    crate::steps::draw(state, player, usize::try_from(amount).unwrap_or(usize::MAX))
}
