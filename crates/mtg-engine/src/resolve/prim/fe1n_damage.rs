//! `Effect::RemoveDamage`: damage marked on creatures is removed (CR 120.6).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent `what` names that is still the object named, on the battlefield and marked with damage loses all of it;
/// one with no damage logs nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RemoveDamage { what } = effect else {
        return Err(wrong_effect("RemoveDamage"));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .filter(|object| {
            object
                .permanent
                .as_ref()
                .is_some_and(|perm| perm.damage() > 0 || perm.deathtouched())
        })
        .map(|object| Event::DamageRemoved {
            object: object.id(),
        })
        .collect();
    Ok(Run::events(events))
}
