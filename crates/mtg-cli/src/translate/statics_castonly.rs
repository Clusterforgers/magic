//! `CantBeCast` on the card itself from every zone: "Cast this spell only if ..." is a casting restriction (CR 601.3).

use serde_json::{Value, json};

use super::ctx::{Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// The casting restriction of a self-only `CantBeCast` line whose condition says when the spell can't be cast; `None` for
/// any other line.
pub fn cast_only(p: &Params, forbidden: Option<&Value>) -> Option<Tr<Value>> {
    let own = p.get("Mode") == Some("CantBeCast")
        && p.get("ValidCard") == Some("Card.Self")
        && p.get("EffectZone") == Some("All");
    own.then(|| restriction(p, forbidden))
}

fn restriction(p: &Params, forbidden: Option<&Value>) -> Tr<Value> {
    check_keys(
        p,
        "Static.CantBeCast",
        &["ValidCard", "Caster", "EffectZone"],
        |key, _| key == "Mode",
    )?;
    if !matches!(p.get("Caster"), None | Some("You")) {
        return unmapped("key:Static.CantBeCast.Caster");
    }
    let Some(forbidden) = forbidden else {
        return unmapped("key:Static.CantBeCast.EffectZone");
    };
    Ok(json!({"CastOnly": {"condition": {"Not": forbidden}}}))
}
