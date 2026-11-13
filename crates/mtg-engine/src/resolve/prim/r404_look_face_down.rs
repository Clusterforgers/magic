//! `Effect::LookAtFaceDown`: the controller looks at face-down permanents (CR 708.5).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The controller looks at each permanent the effect names that is still the object named and lies face down on the
/// battlefield: one `FaceDownLooked` each, in the order named. A face-up permanent, or one that left, is passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::LookAtFaceDown { what } = effect else {
        return Err(wrong_effect("LookAtFaceDown"));
    };
    let state = cx.state();
    let events = cx
        .scope
        .objects(what)?
        .into_iter()
        .filter(|named| {
            state.object(named.id).is_some_and(|object| {
                object.reference() == *named
                    && object.zone() == Zone::Battlefield
                    && object.face_down().is_some()
            })
        })
        .map(|named| Event::FaceDownLooked {
            object: named.id,
            player: cx.controller(),
        })
        .collect();
    Ok(Run::events(events))
}
