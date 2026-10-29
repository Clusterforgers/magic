//! The next upkeep of any player said as the older oracle words it: "Draw a card at the beginning of the next turn's
//! upkeep." The two name the same step, the first upkeep to begin after the effect.

use serde_json::Value;

use crate::ability::Ability;

/// The renderers' words for the step and the oracle's.
const W12A_STEP: [(&str, &str); 2] = [
    (
        "the beginning of the next upkeep",
        "the beginning of the next turn's upkeep",
    ),
    (
        "At the beginning of the next upkeep",
        "At the beginning of the next turn's upkeep",
    ),
];

/// Adds the wordings with "the next turn's upkeep" for an ability with a delayed trigger at the next upkeep of anyone.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    if !w12a_any_upkeep(&value) {
        return;
    }
    let extra: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (from, to) = W12A_STEP.iter().find(|(from, _)| text.contains(from))?;
            Some(text.replace(from, to))
        })
        .collect();
    texts.extend(extra);
}

/// Whether the JSON has a delayed trigger at the beginning of an upkeep with no player named.
fn w12a_any_upkeep(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            let upkeep = map
                .get("DelayedTrigger")
                .and_then(|delayed| delayed.get("when"))
                .and_then(|when| when.get("StepBegins"))
                .is_some_and(|step| {
                    step.get("step").and_then(Value::as_str) == Some("Upkeep")
                        && step.get("whose").is_none()
                });
            upkeep || map.values().any(w12a_any_upkeep)
        }
        Value::Array(items) => items.iter().any(w12a_any_upkeep),
        _ => false,
    }
}
