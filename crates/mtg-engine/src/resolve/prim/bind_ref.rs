//! `Effect::Bind`: the players or objects a reference names now are kept under a name.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, Next, ResolveError, Run};

/// Binds what the reference names as the step runs; a name nothing answers to binds nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Bind { what, bind } = effect else {
        return Err(wrong_effect("Bind"));
    };
    let named = cx.scope.refs(what)?;
    Ok(Run::Done {
        events: Vec::new(),
        bound: vec![(bind.clone(), named)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
