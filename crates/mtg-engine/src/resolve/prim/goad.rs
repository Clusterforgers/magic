//! `Effect::Goad` (CR 701.15): the creatures are goaded by the controller of the effect until that player's next turn.

use mtg_cards::effect::Effect;
use mtg_cards::modify::{Duration, Modification};

use super::{continuous, wrong_effect};
use crate::resolve::{Cx, ResolveError, Run};

/// A continuous effect `Goaded` on the creatures the reference names now (CR 611.2c): the goader is the controller of
/// the effect, and the effect ends when the goader's next turn begins.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Goad { what } = effect else {
        return Err(wrong_effect("Goad"));
    };
    continuous::run(
        cx,
        &Effect::Continuous {
            affected: what.clone(),
            mods: vec![Modification::Goaded],
            duration: Duration::UntilYourNextTurn,
        },
    )
}
