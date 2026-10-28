//! Rules worded from other wordings: a rule that triggers abilities again names its cause as the event of a trigger in
//! the "-ing" form ("If a creature you control dealing combat damage to a player causes ...", "If you casting or copying
//! an instant or sorcery spell causes ..."), and a rule under a condition is the rule's own sentence with "as long as"
//! and the condition at its end ("You may cast this card from your graveyard as long as you control ...").

use serde_json::{Value, json};

use super::super::super::as_long_as;
use super::super::written_variants;
use super::w4d_parts::{heads, wordings};
use super::w5b_parts::w5b_number;
use crate::ability::Ability;
use crate::value::Condition;

/// Adds the wordings of a rule that triggers abilities again or of a rule under a condition.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule, condition, ..
    } = ability
    else {
        return;
    };
    if let Some(condition) = condition {
        let mut bare = ability.clone();
        if let Ability::Rule { condition, .. } = &mut bare {
            *condition = None;
        }
        let mut conditions = as_long_as::texts(condition);
        conditions.extend(at_least(condition));
        for sentence in written_variants(&bare) {
            let Some(sentence) = sentence.strip_suffix('.') else {
                continue;
            };
            for words in &conditions {
                texts.push(format!("{sentence} as long as {words}."));
            }
        }
        return;
    }
    let Ok(Value::Object(rule)) = serde_json::to_value(rule) else {
        return;
    };
    let Some(cause) = rule
        .get("TriggerAgain")
        .and_then(|again| again.get("cause"))
    else {
        return;
    };
    for words in causes(cause) {
        texts.push(format!(
            "If {words} causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
        ));
    }
}

/// The verbs of a trigger's event and their "-ing" forms.
const W5B_GERUNDS: [(&str, &str); 14] = [
    ("deals", "dealing"),
    ("deal", "dealing"),
    ("attacks", "attacking"),
    ("attack", "attacking"),
    ("enters", "entering"),
    ("enter", "entering"),
    ("dies", "dying"),
    ("die", "dying"),
    ("draws", "drawing"),
    ("cast", "casting"),
    ("casts", "casting"),
    ("copy", "copying"),
    ("copies", "copying"),
    ("blocks", "blocking"),
];

/// The event of a trigger in the "-ing" form: "a creature you control dealing combat damage to a player".
fn causes(cause: &Value) -> Vec<String> {
    let trigger = json!({"Triggered": {"when": cause, "effects": [{"Draw": {}}]}});
    let mut found: Vec<String> = heads(&trigger)
        .iter()
        .filter_map(|head| {
            let event = head
                .strip_prefix("Whenever ")
                .or_else(|| head.strip_prefix("When "))?
                .strip_suffix(", ")?;
            let words: Vec<&str> = event
                .split(' ')
                .map(|word| {
                    W5B_GERUNDS
                        .iter()
                        .find(|(verb, _)| *verb == word)
                        .map_or(word, |(_, gerund)| gerund)
                })
                .collect();
            (words.join(" ") != event).then(|| words.join(" "))
        })
        .collect();
    found.dedup();
    found
}

/// "you control three or more tapped Pirates and/or Vehicles": a count of your permanents of at least a number, the
/// permanents named as a count of them names them.
fn at_least(condition: &Condition) -> Vec<String> {
    let Ok(value) = serde_json::to_value(condition) else {
        return Vec::new();
    };
    let Some(compare) = value.get("Compare") else {
        return Vec::new();
    };
    let (Some(count), Some(least)) = (
        compare.get("left").and_then(|left| left.get("Count")),
        compare.get("right").and_then(w5b_number),
    ) else {
        return Vec::new();
    };
    if compare.get("op") != Some(&json!("Ge")) {
        return Vec::new();
    }
    let spell = json!({"Spell": {"effects": [{"Draw": {"count": {"Count": count}}}]}});
    let either = count
        .as_array()
        .is_some_and(|preds| preds.iter().any(|pred| pred.get("AnyOf").is_some()));
    let mut found = Vec::new();
    for text in wordings(spell) {
        let Some(group) = text
            .strip_prefix("Draw cards equal to the number of ")
            .and_then(|rest| rest.strip_suffix(" you control."))
        else {
            continue;
        };
        found.push(format!("you control {least} or more {group}"));
        if either {
            let group = group.replacen(" and ", " and/or ", 1);
            found.push(format!("you control {least} or more {group}"));
        }
    }
    found
}
