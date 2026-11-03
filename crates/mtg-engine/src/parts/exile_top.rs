//! Exiling the top cards of the payer's library as a cost (CR 406): they leave it face up.

use mtg_core::ids::PlayerId;

use super::Payer;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::state::GameState;
use crate::zones::Zone;

/// The cards the payer's library holds.
pub(super) fn most(payer: &Payer<'_>) -> u32 {
    super::mill::most(payer)
}

/// Whether the library holds that many cards: a cost cannot exile what is not there.
pub(super) fn payable(payer: &Payer<'_>, amount: u32) -> bool {
    most(payer) >= amount
}

/// The top cards of the library, the top card first, each a move to exile.
pub(super) fn events(state: &GameState, player: PlayerId, amount: u32) -> Vec<Event> {
    let wanted = usize::try_from(amount).unwrap_or(usize::MAX);
    top_cards(state, player, wanted)
        .into_iter()
        .map(|object| Event::ZoneChanged {
            object,
            from: Zone::Library(player),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        })
        .collect()
}
