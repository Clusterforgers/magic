//! "If <an event> causes a triggered ability of a permanent you control to trigger, that ability triggers an additional
//! time.": the event is the head of a trigger on the cause, its verbs turned into their "-ing" form ("a creature you
//! control dealing combat damage to a player", "you casting or copying an instant or sorcery spell").

use serde_json::json;

use super::w4d_parts::heads;
use crate::ability::Ability;
use crate::modify::GameRule;

/// The verbs of a trigger head and their "-ing" forms.
const W5C_VERBS: [(&str, &str); 22] = [
    ("deals", "dealing"),
    ("deal", "dealing"),
    ("cast", "casting"),
    ("casts", "casting"),
    ("copy", "copying"),
    ("copies", "copying"),
    ("attacks", "attacking"),
    ("attack", "attacking"),
    ("blocks", "blocking"),
    ("block", "blocking"),
    ("enters", "entering"),
    ("enter", "entering"),
    ("dies", "dying"),
    ("die", "dying"),
    ("becomes", "becoming"),
    ("become", "becoming"),
    ("sacrifices", "sacrificing"),
    ("sacrifice", "sacrificing"),
    ("draws", "drawing"),
    ("draw", "drawing"),
    ("activates", "activating"),
    ("activate", "activating"),
];

/// Adds the wording of a rule that makes the triggers of a cause trigger an additional time.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::TriggerAgain { cause },
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    let Ok(when) = serde_json::to_value(cause) else {
        return;
    };
    let trigger = json!({"Triggered": {"when": when, "effects": []}});
    for head in heads(&trigger) {
        let Some(event) = head
            .strip_prefix("Whenever ")
            .and_then(|rest| rest.strip_suffix(", "))
        else {
            continue;
        };
        if let Some(event) = w5c_gerund(event) {
            texts.push(format!(
                "If {event} causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
            ));
        }
    }
}

/// The event with its first verb, and a verb joined to it by "or" or "and", in the "-ing" form.
pub(super) fn w5c_gerund(event: &str) -> Option<String> {
    let mut words: Vec<String> = event.split(' ').map(str::to_owned).collect();
    let first = words.iter().position(|word| w5c_ing(word).is_some())?;
    let mut at = first;
    loop {
        let ing = words.get(at).and_then(|word| w5c_ing(word))?;
        if let Some(word) = words.get_mut(at) {
            *word = ing.to_owned();
        }
        let joined = matches!(words.get(at + 1).map(String::as_str), Some("or" | "and"))
            && words.get(at + 2).and_then(|word| w5c_ing(word)).is_some();
        if !joined {
            break;
        }
        at += 2;
    }
    Some(words.join(" "))
}

/// The "-ing" form of a verb of a trigger head.
fn w5c_ing(word: &str) -> Option<&'static str> {
    W5C_VERBS
        .iter()
        .find(|(verb, _)| *verb == word)
        .map(|(_, ing)| *ing)
}
