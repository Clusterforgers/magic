//! The stack and the order in which players act or order triggers.

use mtg_core::ids::{ObjectId, PlayerId};

use crate::attach;
use crate::events::{Event, ResolutionOutcome};
use crate::loyalty;
use crate::resolve;
use crate::state::GameState;

/// Active-player-nonactive-player order over the players still in the game (CR 101.4).
///
/// Simultaneous triggered abilities will be put on the stack in this order.
pub fn apnap_order(state: &GameState) -> Vec<PlayerId> {
    state.turn_order_from(state.turn().active())
}

/// The events that start resolving the top object of the stack, if any.
///
/// An ability, and a spell with rules to run, begins a resolution that runs its effects step by step; any
/// other spell (a permanent, or a card whose text is left to the table) has no steps and just resolves, and a
/// planeswalker comes in with its loyalty counters in the same batch (CR 306.5b).
pub(crate) fn resolve_top(state: &GameState) -> Option<Vec<Event>> {
    let object = state.object(*state.stack().last()?)?;
    let id = object.id();
    Some(if resolve::runs_effects(state, object) {
        vec![Event::ResolutionBegan { object: id }]
    } else if attach::is_aura_spell(state, id) && attach::spell_host(state, id).is_none() {
        // Its target is no longer legal, so the Aura does not resolve and goes to its owner's graveyard (CR 608.2b).
        vec![
            Event::ResolutionBegan { object: id },
            Event::ResolutionEnded {
                object: id,
                outcome: ResolutionOutcome::Fizzled,
            },
        ]
    } else {
        resolved(state, id)
    })
}

/// A permanent spell resolving: the spell, then the loyalty counters a planeswalker enters with and the host an Aura
/// enters attached to (CR 303.4f).
pub(crate) fn resolved(state: &GameState, id: ObjectId) -> Vec<Event> {
    let mut events = vec![Event::SpellResolved { object: id }];
    events.extend(loyalty::entering(state, id));
    events.extend(attach::entering(state, id));
    events
}
