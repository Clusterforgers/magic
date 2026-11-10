//! wf24 writers for the shapes drafts use most: the translator's name of created tokens (`token`, where the writers of
//! the sentence after a creation know `tokens`), the attachment in the sentence of a creation, and battalion's attack.

use serde_json::Value;

use super::token_binds::TOKENS;
use crate::ability::Ability;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;
use crate::value::Condition;

/// The translator's name of the tokens a creation makes.
const TOKEN: &str = "token";

/// Whether the value uses a string anywhere.
fn uses(value: &Value, wanted: &str) -> bool {
    match value {
        Value::String(text) => text == wanted,
        Value::Array(items) => items.iter().any(|item| uses(item, wanted)),
        Value::Object(map) => map.values().any(|item| uses(item, wanted)),
        _ => false,
    }
}

/// Renames the binding of a creation and the references to it; true when a creation was renamed.
fn rename(value: &mut Value) -> bool {
    match value {
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |found, item| rename(item) || found),
        Value::Object(map) => {
            let mut found = false;
            if map.contains_key("token") && map.get("as").and_then(Value::as_str) == Some(TOKEN) {
                map.insert("as".to_owned(), Value::String(TOKENS.to_owned()));
                found = true;
            }
            if map.len() == 1 && map.get("Bound").and_then(Value::as_str) == Some(TOKEN) {
                map.insert("Bound".to_owned(), Value::String(TOKENS.to_owned()));
            }
            map.values_mut()
                .fold(found, |found, item| rename(item) || found)
        }
        _ => false,
    }
}

/// The ability with its created tokens bound as the writers name them; `None` when nothing is renamed or the writers'
/// name is already in use.
pub(super) fn wf24_tokens_renamed(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    if uses(&value, TOKENS) || !rename(&mut value) {
        return None;
    }
    serde_json::from_value(value).ok()
}

/// The head of a trigger with its condition, for the shapes no other writer knows: battalion, a spell cast during an
/// opponent's turn, and the end of a combat the card fought in; `None` for any other.
pub(super) fn wf24_head(when: &TriggerPattern, condition: Option<&Condition>) -> Option<String> {
    let when_value = serde_json::to_value(when).ok()?;
    let held = condition.and_then(|held| serde_json::to_value(held).ok());
    // "Whenever you cast a spell during an opponent's turn": every turn but yours is an opponent's.
    if when_value == serde_json::json!({"SpellCast": {"by": "You"}})
        && held == Some(serde_json::json!({"AtTrigger": {"Not": "YourTurn"}}))
    {
        return Some("Whenever you cast a spell during an opponent's turn,".to_owned());
    }
    if when_value == serde_json::json!({"StepBegins": {"step": "EndCombat"}}) {
        let fought = serde_json::json!({"Compare": {
            "left": {"Count": ["This", {"AnyOf": [
                {"DidThisTurn": {"event": "Attacked", "combat": true}},
                {"DidThisTurn": {"event": "Blocked", "combat": true}}]}]},
            "op": "Ge", "right": 1}});
        return match held {
            None => Some("At end of combat,".to_owned()),
            Some(held) if held == fought => {
                Some("At end of combat, if ~ attacked or blocked this combat,".to_owned())
            }
            Some(_) => None,
        };
    }
    wf24_attack_with_others(when, condition?)
}

/// Battalion and its kin: "Whenever ~ and at least two other creatures attack", a condition on the attack checked as it
/// triggers; `None` for any other trigger or condition.
fn wf24_attack_with_others(when: &TriggerPattern, condition: &Condition) -> Option<String> {
    let TriggerPattern::Attacks { what } = when else {
        return None;
    };
    if what.0 != [Pred::This] {
        return None;
    }
    let value = serde_json::to_value(condition).ok()?;
    let compare = value.get("AtTrigger")?.get("Compare")?;
    let others = serde_json::json!({"Count": [{"Type": "Creature"}, "Attacking", "Other"]});
    if compare.get("left") != Some(&others)
        || compare.get("op").and_then(Value::as_str) != Some("Ge")
    {
        return None;
    }
    let words = match compare.get("right").and_then(Value::as_u64)? {
        1 => "one other creature",
        2 => "two other creatures",
        3 => "three other creatures",
        _ => return None,
    };
    Some(format!("Whenever ~ and at least {words} attack,"))
}

/// The counters of several sentences put on the one object an earlier sentence named, in one list: "Put a flying
/// counter on it. Put a lifelink counter on it." is "Put a flying counter and a lifelink counter on that creature."
fn counters_listed(text: &str) -> Option<String> {
    let sentences: Vec<&str> = text.split(". ").collect();
    let counters = |sentence: &str| -> Option<String> {
        let rest = sentence.trim_end_matches('.').strip_prefix("Put ")?;
        let counters = rest.strip_suffix(" on it")?;
        (counters.ends_with(" counter") && !counters.contains(" on ")).then(|| counters.to_owned())
    };
    let start = sentences
        .iter()
        .position(|sentence| counters(sentence).is_some())?;
    // "That creature" is the creature a target named before.
    if !sentences[..start].join(". ").contains(" target creature") {
        return None;
    }
    let run: Vec<String> = sentences[start..]
        .iter()
        .map_while(|sentence| counters(sentence))
        .collect();
    let list = match run.as_slice() {
        [first, second] => format!("{first} and {second}"),
        [most @ .., last] if most.len() >= 2 => format!("{}, and {last}", most.join(", ")),
        _ => return None,
    };
    let end = start + run.len();
    let closes = sentences[end - 1].ends_with('.');
    let mut out: Vec<String> = sentences[..start].iter().map(|s| (*s).to_owned()).collect();
    out.push(format!(
        "Put {list} on that creature{}",
        if closes { "." } else { "" }
    ));
    out.extend(sentences[end..].iter().map(|s| (*s).to_owned()));
    Some(out.join(". "))
}

/// "Create a 2/2 Robot artifact creature token and attach ~ to it": the attachment in the sentence of the creation; a
/// creature attached to is also "that creature", and counters on the one object in a list.
pub(super) fn wf24_and_attach(texts: &mut Vec<String>) {
    let mut joined: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" token. Attach ~ to it"))
        .map(|text| text.replacen(" token. Attach ~ to it", " token and attach ~ to it", 1))
        .collect();
    joined.extend(
        texts
            .iter()
            .filter(|text| text.contains(" creature enters, you may attach ~ to it."))
            .map(|text| text.replacen(" attach ~ to it.", " attach ~ to that creature.", 1)),
    );
    joined.extend(texts.iter().filter_map(|text| counters_listed(text)));
    for text in joined {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
