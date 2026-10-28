//! `Effect::Planechase`: "planeswalk" and "chaos ensues" (CR 901) in a game without a planar deck.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, ResolveError, Run};

/// No game the engine plays has a planar deck, so there is no plane to leave and no chaos ability to trigger: the
/// effect is done and logs nothing.
pub(super) fn run(_cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Planechase { .. } = effect else {
        return Err(wrong_effect("Planechase"));
    };
    Ok(Run::events(Vec::new()))
}
