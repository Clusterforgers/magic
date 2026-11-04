//! `MayPlay$ True`: permission to cast or play cards from another zone, as the DSL's `MayCast` rule.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The keys of a `MayPlay` static line.
const READ: [&str; 12] = [
    "Mode",
    "MayPlay",
    "Affected",
    "AffectedZone",
    "MayPlayLimit",
    "MayPlayWithoutManaCost",
    "MayPlayWithFlash",
    "EffectZone",
    "Description",
    "MayPlayPlayer",
    "MayPlayText",
    "Secondary",
];

/// The `MayCast` rule of a static line. `only` names the objects it covers (what an `Effect` object remembers); without
/// it the cards `Affected$` selects are covered.
pub fn may_cast(ctx: &Ctx, p: &Params, only: Option<&Value>) -> Tr<Value> {
    check_keys(p, "Static.MayPlay", &READ, |_, _| false)?;
    let from = match p.get("AffectedZone") {
        Some(zone @ ("Graveyard" | "Exile" | "Library" | "Hand")) => zone,
        Some(other) => return unmapped(format!("zone:MayPlay.AffectedZone={other}")),
        None => return unmapped("key:Static.MayPlay.no-AffectedZone"),
    };
    if !matches!(p.get("EffectZone"), None | Some("Battlefield")) && only.is_none() {
        return unmapped("key:Static.MayPlay.EffectZone");
    }
    let mut permission = json!({ "from": from });
    match (&only, p.get("Affected").unwrap_or("Card")) {
        (Some(objects), "Card.IsRemembered" | "Card.IsRemembered+YouOwn") => {
            permission["only"] = (*objects).clone();
        }
        (Some(_), _) => return unmapped("valid:MayPlay-Affected-with-remembered"),
        (None, valid) => {
            // The zone is the player's own: the ownership is in `from`.
            let what: Vec<Value> = selector_any(ctx, valid)?
                .into_iter()
                .filter(|pred| *pred != json!({"OwnedBy": "You"}))
                .filter(|pred| *pred != json!({"ControlledBy": "You"}))
                .collect();
            if !what.is_empty() {
                permission["what"] = json!(what);
            }
        }
    }
    if p.get("MayPlayWithoutManaCost") == Some("True") {
        permission["free"] = json!(true);
    }
    if p.get("MayPlayWithFlash") == Some("True") {
        permission["flash"] = json!(true);
    }
    match p.get("MayPlayLimit") {
        None => {}
        Some("1") => permission["once"] = json!(true),
        Some(other) => return unmapped(format!("key:Static.MayPlay.MayPlayLimit={other}")),
    }
    match p.get("MayPlayPlayer") {
        None | Some("You") => {}
        Some(other) => return unmapped(format!("key:Static.MayPlay.MayPlayPlayer={other}")),
    }
    // The label of one line of a per-type permission (Muldrotha): each line holds its own `Affected$` and limit.
    let label = p.get("MayPlayText").unwrap_or_default().to_lowercase();
    let text = p.get("Description").unwrap_or_default().to_lowercase();
    if text.contains("play") || label == "land" {
        permission["play"] = json!(true);
    }
    Ok(json!({ "MayCast": permission }))
}
