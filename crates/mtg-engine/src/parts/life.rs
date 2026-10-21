//! Paying life (CR 118.4): a player pays only as much as they have.

use mtg_core::ids::PlayerId;

use super::Payer;
use crate::events::Event;

/// Whether the player has the life.
pub(super) fn payable(payer: &Payer<'_>, amount: u32) -> bool {
    payer
        .state
        .player(payer.player)
        .is_some_and(|seat| i64::from(seat.life()) >= i64::from(amount))
}

/// The life paid; paying nothing is no event.
pub(super) fn events(player: PlayerId, amount: u32) -> Vec<Event> {
    if amount == 0 {
        return Vec::new();
    }
    vec![Event::LifePaid { player, amount }]
}
