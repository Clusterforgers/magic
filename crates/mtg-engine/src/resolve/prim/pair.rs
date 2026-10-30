//! `Effect::Pair`: two creatures become paired (CR 702.95).

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::characteristics::{characteristics, controller_of};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::zones::Zone;

/// The first creature `what` names and the first `with` names become paired, as one `Paired`, when both are still the objects
/// named, on the battlefield under the player who resolves the effect, and paired with nobody (CR 702.95a). Anything else
/// does nothing: a partner that left before the ability resolved, or a creature paired in the meantime.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Pair { what, with } = effect else {
        return Err(wrong_effect("Pair"));
    };
    let first = cx.scope.live_objects(what)?.first().map(|o| o.id());
    let second = cx.scope.live_objects(with)?.first().map(|o| o.id());
    let events = match (first, second) {
        (Some(object), Some(with))
            if object != with
                && free(cx.state(), cx.controller(), object)
                && free(cx.state(), cx.controller(), with) =>
        {
            vec![Event::Paired { object, with }]
        }
        _ => Vec::new(),
    };
    Ok(Run::events(events))
}

/// Whether `id` is a creature on the battlefield that `player` controls and that is paired with no one.
fn free(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|object| object.zone() == Zone::Battlefield)
        && controller_of(state, id) == Some(player)
        && state.partner_of(id).is_none()
        && characteristics(state, id).is_some_and(|chars| chars.is_creature())
}
