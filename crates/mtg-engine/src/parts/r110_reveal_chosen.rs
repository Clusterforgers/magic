//! Revealing a secret choice as a cost ("Reveal the player you chose", CR 706): the source must hold a value it chose
//! secretly; paying shows it to every player, and it stays public.

use mtg_core::ids::ObjectId;

use super::Payer;
use crate::events::Event;
use crate::state::GameState;

/// Whether the source keeps a secret choice not yet revealed.
pub(super) fn payable(payer: &Payer<'_>) -> bool {
    secret(payer.state, payer.source).is_some()
}

/// The reveal of the source's secret choice; nothing when it has none.
pub(super) fn events(state: &GameState, source: ObjectId) -> Vec<Event> {
    secret(state, source)
        .map(|chosen| Event::R110ChoiceRevealed {
            object: source,
            chosen,
        })
        .into_iter()
        .collect()
}

/// The secret choice the permanent keeps.
fn secret(state: &GameState, source: ObjectId) -> Option<crate::object::Chosen> {
    let permanent = state.object(source)?.permanent()?;
    if permanent.r110_secret() {
        permanent.chosen().cloned()
    } else {
        None
    }
}
