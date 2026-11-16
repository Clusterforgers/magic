//! The permanents a permanent devoured (CR 702.82), kept as they last existed on the battlefield so its abilities count
//! those of a kind ("twice the number of Goblins it devoured", `Amount::R6500DevouredMatching`).

use mtg_cards::value::TallyEvent;
use mtg_core::ids::ObjectId;

use crate::object::GameObject;
use crate::state::GameState;

/// Keeps the permanents `object` devoured as their sacrifices logged them, for the permanent it now is.
pub(crate) fn r6500_keep_devoured(state: &mut GameState, object: ObjectId, devoured: &[ObjectId]) {
    let Some(permanent) = state.object(object).map(GameObject::reference) else {
        return;
    };
    if devoured.is_empty() {
        return;
    }
    let shots = devoured
        .iter()
        .filter_map(|id| {
            state
                .tallies
                .logged
                .iter()
                .rev()
                .find(|(kind, _, shot)| *kind == TallyEvent::Sacrificed && shot.id == *id)
                .map(|(_, _, shot)| shot.clone())
        })
        .collect();
    state
        .r5522
        .r6500_devoured
        .retain(|(held, _)| held.id != object);
    state.r5522.r6500_devoured.push((permanent, shots));
}
