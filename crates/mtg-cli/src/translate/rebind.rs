//! A step that moves an object by a bound name names the object again afterwards, as the object it became (CR 400.7).

use serde_json::{Value, json};

/// The verbs whose `what` is a bound name and whose result is a new object.
const MOVERS: [&str; 3] = ["Exile", "Move", "Destroy"];

/// Gives every `Exile`, `Move` and `Destroy` of a bound name that a later step reads again an `as` of the same name, so
/// the later steps see the objects as they are now and not the stale ones.
pub fn rebind(ability: &mut Value) {
    walk(ability, "");
}

fn walk(value: &mut Value, outer_later: &str) {
    match value {
        Value::Array(items) => {
            let texts: Vec<String> = items.iter().map(Value::to_string).collect();
            for index in 0..items.len() {
                let later = format!("{outer_later}{}", texts[index + 1..].concat());
                mark(&mut items[index], &later);
                walk(&mut items[index], &later);
            }
        }
        Value::Object(map) => {
            for child in map.values_mut() {
                walk(child, outer_later);
            }
        }
        _ => {}
    }
}

fn mark(step: &mut Value, later: &str) {
    for verb in MOVERS {
        let Some(body) = step.get_mut(verb) else {
            continue;
        };
        let Some(name) = body
            .get("what")
            .and_then(|what| what.get("Bound"))
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        if body.get("as").is_none() && later.contains(&json!({ "Bound": name }).to_string()) {
            body["as"] = json!(name);
        }
    }
}
