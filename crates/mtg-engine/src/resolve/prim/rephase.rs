//! `Effect::Rephase`: simultaneously, every phased-out creature phases in and every creature with phasing phases out
//! (CR 702.26).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, ResolveError, Run};

/// Both sets are judged before either moves, so a creature with phasing that phases in this way stays in and one that
/// phases out stays out; what is attached goes with each.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Rephase = effect else {
        return Err(wrong_effect("Rephase"));
    };
    Ok(Run::events(crate::phasing::r5514_rephase_events(
        cx.state(),
    )))
}
