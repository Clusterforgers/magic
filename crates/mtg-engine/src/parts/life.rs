//! Paying life (CR 119.4): a player pays only as much as they have.

use mtg_core::ids::PlayerId;

use super::Payer;
use crate::events::Event;

/// The most life the payer can pay: all of it while it is above 0, nothing otherwise, and nothing while their life total
/// can't change (CR 119.8).
pub(super) fn most(payer: &Payer<'_>) -> u32 {
    if crate::rules::life_total_cant_change(payer.state, payer.player) {
        return 0;
    }
    payer
        .state
        .player(payer.player)
        .map_or(0, |seat| u32::try_from(seat.life()).unwrap_or(0))
}

/// Whether the player has the life; paying none is always possible.
pub(super) fn payable(payer: &Payer<'_>, amount: u32) -> bool {
    amount == 0 || most(payer) >= amount
}

/// The life paid; paying nothing is no event.
pub(super) fn events(player: PlayerId, amount: u32) -> Vec<Event> {
    if amount == 0 {
        return Vec::new();
    }
    vec![Event::LifePaid { player, amount }]
}
