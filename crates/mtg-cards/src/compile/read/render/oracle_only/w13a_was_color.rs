//! A condition on the colors a target creature had: "Destroy target creature. If that creature was green or white, its
//! controller discards a card." The condition is written with the first color alone ("if it's a green permanent") and
//! the words are swapped.

use serde_json::{Value, json};

use super::w4d_parts::{capitalized, wordings};
use crate::ability::Ability;

/// Adds the wordings of a spell with one target creature and a step under a condition that it is one of two colors.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let creature = json!([{"what": {"Object": [{"Type": "Creature"}]}}]);
    let Some(inner) = value.get_mut("Spell") else {
        return;
    };
    if inner.get("targets") != Some(&creature) {
        return;
    }
    let Some(effects) = inner.get_mut("effects").and_then(Value::as_array_mut) else {
        return;
    };
    let mut said = None;
    for effect in effects.iter_mut() {
        let Some(condition) = effect
            .get_mut("If")
            .and_then(|step| step.get_mut("condition"))
        else {
            continue;
        };
        if let Some((first, colors)) = w13a_colors(condition) {
            *condition = first;
            said = Some(colors);
        }
    }
    let Some(colors) = said else {
        return;
    };
    let first = colors.split(' ').next().unwrap_or_default();
    let from = format!("if it's a {first} permanent");
    let to = format!("if that creature was {colors}");
    for text in wordings(value) {
        let swapped = text
            .replace(&from, &to)
            .replace(&capitalized(&from), &capitalized(&to));
        if swapped != text {
            texts.push(swapped);
        }
    }
}

/// For a condition that the first target is one of two colors: the condition on the first color alone and the words
/// "green or white".
fn w13a_colors(condition: &Value) -> Option<(Value, String)> {
    let [one, other] = condition.get("Any")?.as_array()?.as_slice() else {
        return None;
    };
    let color = |matches: &Value| -> Option<String> {
        let matches = matches.get("Matches")?;
        if matches.get("subject")? != &json!({"Target": 0}) {
            return None;
        }
        match matches.get("selector")?.as_array()?.as_slice() {
            [pred] => pred.get("Color")?.as_str().map(str::to_lowercase),
            _ => None,
        }
    };
    let (first, second) = (color(one)?, color(other)?);
    Some((one.clone(), format!("{first} or {second}")))
}
