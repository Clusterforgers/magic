//! Naming what has no writer: which part of an ability the renderers cannot write.

use serde_json::{Value, json};

use crate::ability::Ability;
use crate::compile::written_variants;

/// An event that the renderers write, to test the parts of an ability that come after it.
fn known_when() -> Value {
    json!({ "ZoneChange": { "what": ["This"], "to": "Battlefield" } })
}

/// The variant name of an externally tagged value.
fn variant(value: &Value) -> String {
    match value {
        Value::String(name) => name.clone(),
        Value::Object(map) => map.keys().next().cloned().unwrap_or_default(),
        _ => "?".to_owned(),
    }
}

/// The ability's variant name.
pub(super) fn name(ability: &Ability) -> String {
    serde_json::to_value(ability).map_or_else(|_| "ability".to_owned(), |value| variant(&value))
}

/// Whether the renderers write the ability described by a JSON value.
fn writes(value: &Value) -> bool {
    serde_json::from_value::<Ability>(value.clone())
        .is_ok_and(|ability| !written_variants(&ability).is_empty())
}

/// The ability and the part of it that has no writer, found by writing each part with a known-good frame. A
/// part is named by its variant; when every part writes alone the combination is what has no writer.
pub(super) fn construct(ability: &Ability) -> String {
    let Ok(value) = serde_json::to_value(ability) else {
        return name(ability);
    };
    let head = variant(&value);
    let Some(body) = value.get(&head).and_then(Value::as_object) else {
        return head;
    };
    let parts = match head.as_str() {
        "Spell" | "Triggered" | "Activated" | "Loyalty" => action_parts(&head, body),
        "Static" => static_parts(body),
        _ => Vec::new(),
    };
    if parts.is_empty() {
        format!("{head} (the combination or its shape)")
    } else {
        format!("{head} ({})", parts.join("; "))
    }
}

/// The parts of a spell, a triggered or an activated ability that have no writer.
fn action_parts(head: &str, body: &serde_json::Map<String, Value>) -> Vec<String> {
    let draw = json!([{ "Draw": {} }]);
    let get = |key: &str| body.get(key).cloned();
    let mut parts = Vec::new();
    let frame = |when: Option<Value>, effects: Value| {
        let mut frame = serde_json::Map::new();
        if head == "Triggered" {
            frame.insert("when".to_owned(), when.unwrap_or_else(known_when));
        } else if head == "Activated" {
            frame.insert("cost".to_owned(), json!(["Tap"]));
        } else if head == "Loyalty" {
            frame.insert("cost".to_owned(), json!("Zero"));
        }
        frame.insert("effects".to_owned(), effects);
        json!({ head: frame })
    };
    if head == "Triggered" {
        let mut probe = frame(get("when"), draw.clone());
        if let Some(held) = get("intervening_if") {
            probe[head]["intervening_if"] = held;
        }
        if !writes(&probe) {
            let name = get("when").map(|when| variant(&when)).unwrap_or_default();
            parts.push(format!("trigger {name} or its condition"));
        }
    }
    if head == "Activated" {
        let mut probe = frame(None, draw);
        for key in ["cost", "timing", "condition", "limit", "from"] {
            if let Some(part) = get(key) {
                probe[head][key] = part;
            }
        }
        if !writes(&probe) {
            parts.push("cost, timing, condition or limit".to_owned());
        }
    }
    let effects = get("effects").and_then(|list| list.as_array().cloned());
    for effect in effects.into_iter().flatten() {
        let mut probe = frame(None, json!([effect]));
        if let Some(targets) = get("targets") {
            probe[head]["targets"] = targets;
        }
        if !writes(&probe) {
            parts.push(format!("effect {}", variant(&effect)));
        }
    }
    parts
}

/// The modifications of a static ability that have no writer.
fn static_parts(body: &serde_json::Map<String, Value>) -> Vec<String> {
    let mods = body.get("mods").and_then(Value::as_array);
    mods.into_iter()
        .flatten()
        .filter(|modification| {
            let mut probe =
                json!({ "Static": { "affected": body.get("affected"), "mods": [modification] } });
            if let Some(held) = body.get("condition") {
                probe["Static"]["condition"] = held.clone();
            }
            !writes(&probe)
        })
        .map(|modification| format!("modification {}", variant(modification)))
        .collect()
}
