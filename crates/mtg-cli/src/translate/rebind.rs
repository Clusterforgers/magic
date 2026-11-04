//! An exile of a bound object names the exiled object again afterwards, as the object it became (CR 400.7).

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
        Value::Object(map) => map.values_mut().for_each(rebind),
        _ => {}
    }
}

/// Names the objects an `Exile` of a bound name exiles when `later` reads the old name; the old and the new name.
fn mark(step: &mut Value, later: &[Value]) -> Option<(String, String)> {
    let body = step.get_mut("Exile")?;
    let old = body.get("what")?.get("Bound")?.as_str()?.to_owned();
    let read = json!({ "Bound": old }).to_string();
    if body.get("as").is_some() || !later.iter().any(|step| step.to_string().contains(&read)) {
        return None;
    }
    let new = format!("{old}_exiled");
    body["as"] = json!(new);
    Some((old, new))
}

fn rename(value: &mut Value, old: &str, new: &str) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| rename(item, old, new)),
        Value::Object(map) => {
            if map.len() == 1 && map.get("Bound").and_then(Value::as_str) == Some(old) {
                map.insert("Bound".to_owned(), json!(new));
            } else {
                map.values_mut().for_each(|child| rename(child, old, new));
            }
        }
        _ => {}
    }
}
