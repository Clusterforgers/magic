//! `Effect::May` (a player may do the effects): a stub until the task that plays it (`docs/cards-dsl.md`, Tasks).

use mtg_cards::effect::Effect;

use super::unsupported;
use crate::resolve::{Cx, ResolveError, Run};

/// What the interpreter does with the effect: nothing yet, and `Caps` does not name it.
pub(super) fn run(_cx: &Cx<'_>, _effect: &Effect) -> Result<Run, ResolveError> {
    unsupported("Effect.May")
}
