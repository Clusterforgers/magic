//! "If you do" and "If you don't" asked of what an optional step bound (r5b): `you may sacrifice a creature` binds the
//! sacrificed creature, and an `If` that counts that binding at one or more (or at none) right after the step is the
//! `If` of `Performed` (or of its negation) the writers word. The ability is rewritten to that form and worded again,
//! so only the step that binds the name, directly before the `If`, links it.

use serde_json::{Value, json};

use crate::ability::Ability;

/// The wordings of the ability with each such `If` rewritten to `Performed`, or nothing when none is.
pub(super) fn variants(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return Vec::new();
    };
    if !rewrite(&mut value) {
        return Vec::new();
    }
    serde_json::from_value::<Ability>(value).map_or_else(|_| Vec::new(), |normal| written(&normal))
}

/// Rewrites every effect list of the ability; whether one changed.
fn rewrite(value: &mut Value) -> bool {
    let mut changed = false;
    match value {
        Value::Object(map) => {
            for (key, inner) in map.iter_mut() {
                if key == "effects"
                    && let Value::Array(list) = inner
                {
                    changed |= rewrite_list(list);
                }
                changed |= rewrite(inner);
            }
        }
        Value::Array(list) => {
            for inner in list {
                changed |= rewrite(inner);
            }
        }
        _ => {}
    }
    changed
}

/// `[May [.., step], If]` and `May [.., step, If]` over the name the step binds.
fn rewrite_list(list: &mut Vec<Value>) -> bool {
    let mut changed = false;
    // The `If` last inside a `May`, right after the step: moved after the `May`.
    for at in 0..list.len() {
        let Some(inner) = may_effects(&mut list[at]) else {
            continue;
        };
        let [.., step, last] = inner.as_slice() else {
            continue;
        };
        if performed(step, last).is_some() {
            let Some(last) = inner.pop() else { continue };
            list.insert(at + 1, last);
            changed = true;
            break;
        }
    }
    // The step of the `May` before, kept across the `If`s that follow it ("If you do, S. If you don't, T.").
    let mut step = None;
    for at in 0..list.len() {
        if let Some(last) = may_last(&list[at]) {
            step = Some(last);
            continue;
        }
        let Some(condition) = step.as_ref().and_then(|step| performed(step, &list[at])) else {
            step = None;
            continue;
        };
        list[at]["If"]["condition"] = condition;
        changed = true;
    }
    changed
}

fn may_effects(value: &mut Value) -> Option<&mut Vec<Value>> {
    let may = value.get_mut("May")?;
    // Only "you may": another player's choice is worded with its player.
    if may.get("who").is_some_and(|who| who != "You") {
        return None;
    }
    may.get_mut("effects")?.as_array_mut()
}

fn may_last(value: &Value) -> Option<Value> {
    let may = value.get("May")?;
    if may.get("who").is_some_and(|who| who != "You") {
        return None;
    }
    may.get("effects")?.as_array()?.last().cloned()
}

/// The steps whose binding counts what they took away.
const STEPS: [&str; 4] = ["Sacrifice", "Discard", "Exile", "Mill"];

/// `"Performed"` or `{"Not":"Performed"}` when the `If` asks whether the step took anything, with no `otherwise`.
fn performed(step: &Value, branch: &Value) -> Option<Value> {
    let (verb, body) = step.as_object()?.iter().next()?;
    STEPS.contains(&verb.as_str()).then_some(())?;
    let name = body.get("as").or_else(|| body.get("bind"))?.as_str()?;
    let branch = branch.get("If")?;
    if branch
        .get("otherwise")
        .and_then(Value::as_array)
        .is_some_and(|list| !list.is_empty())
    {
        return None;
    }
    let condition = branch.get("condition")?;
    let single = body.get("count").is_none();
    let did = if let Some(count) = condition.get("CountOf") {
        (count.get("subject")? == &json!({ "Bound": name })).then_some(())?;
        let selector = count.get("selector")?;
        let same_kind = selector == &json!([]) || Some(selector) == body.get("filter");
        same_kind.then_some(())?;
        did(count.get("op")?, count.get("right")?, single)?
    } else if let Some(compare) = condition.get("Compare") {
        (compare.get("left")? == &json!({ "Bound": format!("{name}~moved") })).then_some(())?;
        did(compare.get("op")?, compare.get("right")?, single)?
    } else {
        return None;
    };
    Some(if did {
        json!("Performed")
    } else {
        json!({ "Not": "Performed" })
    })
}

/// Whether the comparison of the count asks "one or more" (true) or "none" (false).
fn did(op: &Value, right: &Value, single: bool) -> Option<bool> {
    match (op.as_str()?, right.as_i64()?) {
        ("Ge", 1) | ("Gt", 0) => Some(true),
        // One step that takes one thing took one or none.
        ("Eq", 1) if single => Some(true),
        ("Eq" | "Le", 0) | ("Lt", 1) => Some(false),
        _ => None,
    }
}
