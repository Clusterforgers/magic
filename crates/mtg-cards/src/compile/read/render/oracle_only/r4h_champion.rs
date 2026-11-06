//! Champion (CR 702.72): "Champion a Goblin" stands for a trigger that sacrifices the creature unless it exiles another
//! creature of the type for as long as it stays. The data is that one trigger; the renderers write its steps, this
//! writes the keyword line the card prints.

use crate::ability::Ability;
use serde_json::{Value, json};

/// The trigger the keyword stands for, over the creatures `choices` names.
fn trigger(choices: Value) -> Value {
    json!({"Triggered": {
        "when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}},
        "effects": [
            {"Choose": {"from": {"Each": choices}, "count": {"min": 0, "max": 1}, "as": "champion"}},
            {"Exile": {"what": {"Bound": "champion"}, "duration": "WhileSourceOnBattlefield"}},
            {"If": {"condition": {"Not": "Performed"}, "then": [{"Sacrifice": {"filter": ["This"]}}]}}]}})
}

/// The type words of the trigger's choice ("creature", or the subtypes), `None` when the ability is not a champion.
fn words(value: &Value) -> Option<Vec<String>> {
    let first = value.get("Triggered")?.get("effects")?.get(0)?;
    let each = first.get("Choose")?.get("from")?.get("Each")?.as_array()?;
    let kind = each.first()?;
    let named = |pred: &Value| -> Option<String> {
        if pred.get("Type").and_then(Value::as_str) == Some("Creature") {
            return Some("creature".to_owned());
        }
        pred.get("Subtype")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let words: Vec<String> = match kind.get("AnyOf") {
        Some(any) => any.as_array()?.iter().map(named).collect::<Option<_>>()?,
        None => vec![named(kind)?],
    };
    let any_of = |words: &[String]| match words {
        [one] if one == "creature" => {
            json!([{"Type": "Creature"}, "Other", {"ControlledBy": "You"}])
        }
        [one] => json!([{"Subtype": one}, "Other", {"ControlledBy": "You"}]),
        many => json!([
            {"AnyOf": many.iter().map(|word| json!({"Subtype": word})).collect::<Vec<_>>()},
            "Other",
            {"ControlledBy": "You"}]),
    };
    (trigger(any_of(&words)) == *value && !words.is_empty()).then_some(words)
}

/// "a Goblin", "an Elf", "a creature".
fn with_article(word: &str) -> String {
    let article = if word.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {word}")
}

/// Adds the keyword line of a champion trigger.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(words) = words(&value) else {
        return;
    };
    let Some((first, rest)) = words.split_first() else {
        return;
    };
    let mut line = format!("Champion {}", with_article(first));
    for word in rest {
        line.push_str(" or ");
        line.push_str(word);
    }
    texts.push(line);
}
