//! The stack and the order in which players act or order triggers.

use mtg_core::ids::PlayerId;

use crate::events::Event;
use crate::state::GameState;

/// Active-player-nonactive-player order over the players still in the game (CR 101.4).
///
/// Simultaneous triggered abilities will be put on the stack in this order.
pub fn apnap_order(state: &GameState) -> Vec<PlayerId> {
    state.turn_order_from(state.turn().active())
}

/// The event that resolves the top object of the stack, if any.
pub(crate) fn resolve_top(state: &GameState) -> Option<Event> {
    state
        .stack()
        .last()
        .map(|object| Event::SpellResolved { object: *object })
}
