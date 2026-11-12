//! The turn's log of permanents destroyed and spells countered by a spell or ability, for the player who controlled it
//! (`TallyEvent::Destroyed`, `TallyEvent::Countered`): "if a creature spell you cast this turn was countered by a spell or
//! ability an opponent controlled".

use mtg_cards::value::TallyEvent;
use mtg_core::ids::ObjectId;

use crate::events::ZoneCause;
use crate::object::GameObject;
use crate::state::GameState;
use crate::zones::Zone;

/// Logs the object a resolving spell or ability destroys or counters, as it is before the move, for the player who controls
/// what resolves; a destruction or a counter with no spell or ability resolving (a state-based action) logs nothing.
pub(super) fn log_ruin(state: &mut GameState, cause: ZoneCause, from: Zone, id: ObjectId) {
    let event = match (cause, from) {
        (ZoneCause::Destroyed | ZoneCause::Buried, Zone::Battlefield) => TallyEvent::Destroyed,
        (ZoneCause::Countered, Zone::Stack) => TallyEvent::Countered,
        _ => return,
    };
    let Some(player) = state
        .resolving
        .as_ref()
        .filter(|resolving| resolving.object != id)
        .and_then(|resolving| state.object(resolving.object))
        .map(GameObject::controller)
    else {
        return;
    };
    state.log_event(event, player, id);
}
