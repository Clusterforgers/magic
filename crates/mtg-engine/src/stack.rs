//! The stack and the order in which players act or order triggers.

use mtg_core::ids::PlayerId;

use crate::events::Event;
use crate::resolve;
use crate::state::GameState;

/// Active-player-nonactive-player order over the players still in the game (CR 101.4).
///
/// Simultaneous triggered abilities will be put on the stack in this order.
pub fn apnap_order(state: &GameState) -> Vec<PlayerId> {
    state.turn_order_from(state.turn().active())
}

/// The event that starts resolving the top object of the stack, if any.
///
/// An ability, and a spell with rules to run, begins a resolution that runs its effects step by step; any
/// other spell (a permanent, or a card whose text is left to the table) has no steps and just resolves.
pub(crate) fn resolve_top(state: &GameState) -> Option<Event> {
    let object = state.object(*state.stack().last()?)?;
    Some(if resolve::runs_effects(object) {
        Event::ResolutionBegan {
            object: object.id(),
        }
    } else {
        Event::SpellResolved {
            object: object.id(),
        }
    })
}
