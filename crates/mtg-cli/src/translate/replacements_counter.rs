//! "Can't be countered": an `Event$ Counter` replacement with `Layer$ CantHappen` as the rule `CantBeCountered`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// A `Counter` event line: the spell itself (from the stack) or the group of spells it names (from the battlefield).
pub fn counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    let read = ["Event", "Layer", "ValidSA", "ValidCard", "ActiveZones"];
    check_keys(p, "Replacement.Counter", &read, |_, _| false)?;
    if p.get("Layer") != Some("CantHappen") {
        return unmapped("replacement:Counter-instead");
    }
    // An ability is not a spell: a rule over abilities stays a gap.
    let sa = p.get("ValidSA").unwrap_or_default();
    if sa
        .split(',')
        .any(|one| one.split('.').next() != Some("Spell"))
    {
        return unmapped("replacement:Counter-abilities");
    }
    let zone = p.get("ActiveZones");
    if p.get("ValidCard") == Some("Card.Self") && sa == "Spell" {
        if zone.is_some() {
            return unmapped("replacement:Counter-zone");
        }
        return Ok(json!({"Rule": {"rule": {"CantBeCountered": "This"}, "from": "Stack"}}));
    }
    if zone != Some("Battlefield") {
        return unmapped("replacement:Counter-zone");
    }
    // `ValidCard` is the countered card itself: it names the group only beside a plain `Spell`.
    let spec = match (p.get("ValidCard"), sa) {
        (None, sa) => sa,
        (Some(card), "Spell") => card,
        _ => return unmapped("replacement:Counter-shape"),
    };
    let mut preds = selector_any(ctx, spec)?;
    if p.get("ValidCard").is_some() {
        preds.insert(0, json!("Spell"));
    }
    Ok(json!({"Rule": {"rule": {"CantBeCountered": {"Each": preds}}}}))
}
