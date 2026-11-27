//! The end of a copy effect that lasts until the next end step (CR 513, 611.2b): it ends as that step begins, before
//! the abilities that trigger at its beginning are put on the stack.

use crate::copies::C10XpCopyEnds;
use crate::events::Event;
use crate::state::GameState;

/// The `CopyEnded` events as an end step begins: each permanent under a copy effect until the next end step gives way
/// to what it was before.
pub(crate) fn r513_copies_ending(state: &GameState) -> Vec<Event> {
    state
        .objects()
        .filter(|object| {
            object
                .copy_effect()
                .is_some_and(|copy| copy.c10_xp_ends_at(C10XpCopyEnds::R513EndStep))
        })
        .map(|object| Event::CopyEnded {
            object: object.id(),
            next_turn_of: None,
            attached_by: None,
            r6104_tapped: None,
            r707_static: None,
            r513_end_step: true,
        })
        .collect()
}
