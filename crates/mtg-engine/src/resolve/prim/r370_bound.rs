//! The targets a damage shield's effects read once the ability that made it is gone: each is fixed as the shield
//! begins, under the name `Ref::Target` reads it by (CR 611.2c), so "if damage is prevented this way, ~ deals that much
//! damage to any target" deals it to the target chosen as the spell was cast.

use mtg_cards::refs::Ref;

use crate::choose::Target;
use crate::resolve::{Cx, ResolveError, W7D_LEGAL};

/// The targets of the resolving ability, each under the name its index is read by, as they are now.
pub(super) fn r370_shield_targets(cx: &Cx<'_>) -> Result<Vec<(String, Vec<Target>)>, ResolveError> {
    let count = cx
        .scope
        .choices
        .targets
        .len()
        .saturating_sub(cx.scope.target_base);
    let mut bound = Vec::new();
    for index in 0..count.min(usize::from(u8::MAX)) {
        let Ok(at) = u8::try_from(index) else {
            break;
        };
        bound.push((
            format!("{W7D_LEGAL}{index}"),
            cx.scope.refs(&Ref::Target(at))?,
        ));
    }
    Ok(bound)
}
