//! The end of a copy effect that lasts until its controller's next turn (CR 611.2b, 707.2).

use mtg_core::ids::PlayerId;

use crate::copies::C10XpCopyEnds;
use crate::events::Event;
use crate::state::GameState;

/// The `CopyEnded` events as `active`'s turn begins: each permanent under a copy effect until the next turn of
/// `active`, or of a player who has left the game, gives way to what it was before (CR 800.4a).
pub(crate) fn c10_xp_copies_ending(state: &GameState, active: PlayerId) -> Vec<Event> {
    let mut events = Vec::new();
    for object in state.objects() {
        let Some(copy) = object.copy_effect() else {
            continue;
        };
        for player in state.players().iter().map(|p| p.id) {
            let ends_now = player == active || !state.is_alive(player);
            if ends_now && copy.c10_xp_ends_at(C10XpCopyEnds::NextTurnOf(player)) {
                events.push(Event::CopyEnded {
                    object: object.id(),
                    next_turn_of: Some(player),
                });
            }
        }
    }
    events
}
