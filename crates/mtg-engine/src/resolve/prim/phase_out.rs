//! `Effect::PhaseOut`: permanents phase out (CR 702.26).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the effect names that is on the battlefield phases out, all together in the order the reference lists
/// them, and the Auras and Equipment attached to it phase out with it. A permanent that is phased out already, an object
/// that is not a permanent or has changed zones since it was named (CR 400.7) and a name that repeats are passed over,
/// and naming nothing phases nothing out.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::PhaseOut { what } = effect else {
        return Err(wrong_effect("PhaseOut"));
    };
    let named: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(|object| object.id())
        .collect();
    Ok(Run::events(crate::phasing::out_events(cx.state(), &named)))
}
