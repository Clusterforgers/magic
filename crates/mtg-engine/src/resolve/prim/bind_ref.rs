//! `Effect::Bind`: the players or objects a reference names now are kept under a name.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::resolve::{Cx, Next, ResolveError, Run};

/// Binds what the reference names as the step runs; a name nothing answers to binds nothing. An adding bind keeps what
/// the name held before it, then what the reference names that was not held yet.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Bind { what, bind, adds } = effect else {
        return Err(wrong_effect("Bind"));
    };
    let held = cx.resolving.bindings().get(bind).filter(|_| *adds);
    let mut named = held.cloned().unwrap_or_default();
    for found in cx.scope.refs(what)? {
        if !named.contains(&found) {
            named.push(found);
        }
    }
    // An adding bind that names nothing new leaves the name as it is.
    let bound = if held.is_some_and(|held| held.len() == named.len()) {
        Vec::new()
    } else {
        vec![(bind.clone(), named)]
    };
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
