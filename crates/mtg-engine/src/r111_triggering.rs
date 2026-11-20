//! `Ref::Triggering` of an ability that triggered on another triggered ability triggering (CR 603.2): that ability on
//! the stack ("counter that ability", "copy that ability"), found by its address and the event it triggered on.

use crate::abilities::Cause;
use crate::choose::Target;
use crate::state::GameState;
use crate::zones::Zone;

/// What `Ref::Triggering` names for `cause`: the triggered ability it is about while that ability is on the stack
/// (nothing once it has left), else the subjects of the event.
pub(crate) fn r111_subjects(state: &GameState, cause: &Cause) -> Vec<Target> {
    let Some(address) = cause.ability else {
        return cause.subjects.clone();
    };
    state
        .zone(Zone::Stack)
        .iter()
        .filter_map(|&id| state.object(id))
        .filter(|object| {
            object.ability().is_some_and(|ability| {
                ability.source().id == address.object
                    && ability.index() == address.ability
                    && ability.origin() == address.source
                    && ability.cause().is_some_and(|own| own.event == cause.event)
            })
        })
        .map(|object| Target::Object(object.reference()))
        .collect()
}
