//! `Effect::PutBeneathTop`: objects go into their owners' libraries just beneath the top cards of a counted amount.

use mtg_cards::effect::{Destination, Effect, Place};

use super::{move_to, wrong_effect};
use crate::resolve::{Cx, ResolveError, Run};

/// Works out how many cards stay above (none below zero) and moves the objects as a move to that place in the library
/// does: each into its owner's library, a library with fewer cards taking them at the bottom.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::PutBeneathTop { what, below } = effect else {
        return Err(wrong_effect("PutBeneathTop"));
    };
    let below = u32::try_from(cx.scope.amount(below)?.max(0)).unwrap_or(u32::MAX);
    let place = if below == 0 {
        Place::Top
    } else {
        Place::Below(below)
    };
    let moved = Effect::Move {
        what: what.clone(),
        to: Destination::Library { place, order: None },
        bind: None,
    };
    move_to::run(cx, &moved)
}
