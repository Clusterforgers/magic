//! `Effect::Let`: the amount is worked out once and kept under a name (`Amount::Bound`).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, LET, Next, ResolveError, Run};

/// Binds the number the amount has now as `~let:<number>:<name>`; the finished step stores it with the resolution, so the
/// steps after it read the same number however the game changes and a replay of the log rebuilds it.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Let { bind, amount } = effect else {
        return Err(wrong_effect("Let"));
    };
    let value = cx.scope.amount(amount)?;
    Ok(Run::Done {
        events: Vec::new(),
        bound: vec![(format!("{LET}{value}:{bind}"), Vec::new())],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
