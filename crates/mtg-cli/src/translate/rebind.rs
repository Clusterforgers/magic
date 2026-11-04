//! An exile of a bound object or a target names the exiled object again afterwards, as the object it became (CR 400.7); a sacrifice of a bound object takes all of it.

use serde_json::{Value, json};

/// Gives every `Exile` of a bound name that a later step of the same list reads again an `as` with a fresh name and makes
/// those steps read that name, so they see the exiled objects and not the stale ones. A step inside a branch binds only
/// the steps after it in that branch: the other branches would read a name nothing bound.
pub fn rebind(ability: &mut Value) {
    match ability {
        Value::Array(items) => {
            for index in 0..items.len() {
                let (head, tail) = items.split_at_mut(index + 1);
                if let Some((old, new)) = mark(&mut head[index], tail) {
                    for after in tail {
                        rename(after, &old, &new);
                    }
                }
            }
            items.iter_mut().for_each(rebind);
        }
        Value::Object(map) => {
            if let Some(body) = map.get_mut("Sacrifice") {
                sacrifice_all_of_bound(body);
            }
            map.values_mut().for_each(rebind);
        }
        _ => {}
    }
}

/// A `Sacrifice` filtered by `Is` one bound name, without a count, takes every object of that name.
fn sacrifice_all_of_bound(body: &mut Value) {
    let Some(map) = body.as_object_mut() else {
        return;
    };
    if map.contains_key("count") {
        return;
    }
    let Some(object) = map
        .get("filter")
        .and_then(Value::as_array)
        .filter(|filter| filter.len() == 1)
        .and_then(|filter| filter[0].get("Is"))
        .filter(|object| object.get("Bound").is_some())
        .cloned()
    else {
        return;
    };
    map.insert("count".to_owned(), json!({"Count": [{"Is": object}]}));
}

/// Names the objects an `Exile` of a bound name or of a target exiles when `later` reads the old name; the old reference
/// and the new name. A target is read again only by the steps that act on the cards (`Move`, `SeparatePiles`): a step that
/// reads a stat or the controller keeps the last-known object of the target.
fn mark(step: &mut Value, later: &[Value]) -> Option<(Value, String)> {
    let body = step.get_mut("Exile")?;
    let what = body.get("what")?.clone();
    let old = match (
        what.get("Bound").and_then(Value::as_str),
        what.get("Target"),
    ) {
        (Some(name), _) => name.to_owned(),
        (None, Some(index)) if what.as_object()?.len() == 1 => format!("target{index}"),
        _ => return None,
    };
    let reads = match what.get("Target") {
        Some(_) => later.iter().any(|step| acts_on(step, &what)),
        None => later
            .iter()
            .any(|step| step.to_string().contains(&what.to_string())),
    };
    if body.get("as").is_some() || !reads {
        return None;
    }
    let new = format!("{old}_exiled");
    body["as"] = json!(new);
    Some((what, new))
}

/// Whether the step, or one inside it, is a `Move` or `SeparatePiles` of exactly `what`.
fn acts_on(value: &Value, what: &Value) -> bool {
    match value {
        Value::Array(items) => items.iter().any(|item| acts_on(item, what)),
        Value::Object(map) => {
            ["Move", "SeparatePiles"]
                .iter()
                .any(|verb| map.get(*verb).and_then(|body| body.get("what")) == Some(what))
                || map.values().any(|child| acts_on(child, what))
        }
        _ => false,
    }
}

/// Makes the steps that read `old` read `new`: every reading of a bound name, only the acting steps for a target.
fn rename(value: &mut Value, old: &Value, new: &str) {
    if old.get("Target").is_some() {
        if let Value::Object(map) = value {
            for verb in ["Move", "SeparatePiles"] {
                if let Some(body) = map.get_mut(verb)
                    && body.get("what") == Some(old)
                {
                    body["what"] = json!({ "Bound": new });
                }
            }
        }
        match value {
            Value::Array(items) => items.iter_mut().for_each(|item| rename(item, old, new)),
            Value::Object(map) => map.values_mut().for_each(|child| rename(child, old, new)),
            _ => {}
        }
        return;
    }
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| rename(item, old, new)),
        Value::Object(map) => {
            if map.len() == 1 && map.get("Bound") == old.get("Bound") {
                map.insert("Bound".to_owned(), json!(new));
            } else {
                map.values_mut().for_each(|child| rename(child, old, new));
            }
        }
        _ => {}
    }
}
