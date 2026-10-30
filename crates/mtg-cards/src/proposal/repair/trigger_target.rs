//! `Triggering` in a spell, activated or loyalty ability whose declared targets are all used is the target the printed
//! "target ..." phrase declares: the ability has no trigger, so the model meant "that creature", the chosen target.

use serde_json::Value;

use super::targets::{scoped, trigger_to_target, used_by};
use super::{Repair, RepairClass, Site, body_of, declare};

/// The abilities whose effects can name a target and have no event of their own.
const KINDS: [&str; 3] = ["Spell", "Activated", "Loyalty"];

/// Whether the value names the triggering object.
fn names_triggering(value: &Value) -> bool {
    match value {
        Value::String(word) => word == "Triggering",
        Value::Object(map) => map.values().any(names_triggering),
        Value::Array(items) => items.iter().any(names_triggering),
        _ => false,
    }
}

/// Makes the `Triggering` of the effects a new target when the printed phrases declare it.
pub(super) fn repair(ability: &mut Value, site: &Site<'_>, repairs: &mut Vec<Repair>) {
    let Some((name, body)) = body_of(ability) else {
        return;
    };
    if !KINDS.contains(&name) || body.values().any(scoped) {
        return;
    }
    if !body.get("effects").is_some_and(names_triggering) {
        return;
    }
    let declared = body
        .get("targets")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    // A declared target that no effect uses is for the `Triggering` already (see `targets`).
    if used_by(body).len() != declared {
        return;
    }
    let index = u64::try_from(declared).unwrap_or(u64::MAX);
    let mut trial = ability.clone();
    let replaced = body_of(&mut trial)
        .and_then(|(_, body)| body.get_mut("effects"))
        .map_or(0, |effects| trigger_to_target(effects, index));
    let mut declaring = Vec::new();
    declare::repair(&mut trial, site, &mut declaring);
    // The new target must be one the printed phrases declare: otherwise the `Triggering` stays as it is.
    if replaced == 0 || declaring.is_empty() {
        return;
    }
    *ability = trial;
    repairs.push(Repair {
        class: RepairClass::TriggeringDeclared,
        at: site.at.to_owned(),
        note: format!("`Triggering` outside a triggered ability is the target {index}, which the printed phrase declares"),
    });
    repairs.append(&mut declaring);
}
