//! A group under the condition that it has exactly one member, named "that creature": "As long as you control exactly
//! one creature, that creature gets +3/+1 and has lifelink." The static is written with the count at least one ("as
//! long as you control a creature, each creature you control gets ...") and the words are swapped.

use serde_json::json;

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of a static on the creatures the controller controls while the count of them is exactly one.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let group = json!([{"Type": "Creature"}, {"ControlledBy": "You"}]);
    let Some(inner) = value.get_mut("Static") else {
        return;
    };
    if inner.get("affected").and_then(|affected| affected.get("Each")) != Some(&group) {
        return;
    }
    let Some(compare) = inner
        .get_mut("condition")
        .and_then(|condition| condition.get_mut("Compare"))
    else {
        return;
    };
    let exactly_one = compare.get("left") == Some(&json!({"Count": group}))
        && compare.get("op") == Some(&json!("Eq"))
        && compare.get("right") == Some(&json!(1));
    if !exactly_one {
        return;
    }
    compare["op"] = json!("Ge");
    for text in wordings(value) {
        let Some(rest) = text.strip_prefix("As long as you control a creature, each creature you control ") else {
            continue;
        };
        texts.push(format!(
            "As long as you control exactly one creature, that creature {rest}"
        ));
    }
}

