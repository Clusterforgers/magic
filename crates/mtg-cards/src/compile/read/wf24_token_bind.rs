//! wf24 writers for the shapes drafts use most: the translator's name of created tokens (`token`, where the writers of
//! the sentence after a creation know `tokens`), the attachment in the sentence of a creation, and battalion's attack.

use serde_json::Value;

use super::token_binds::TOKENS;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref};
use crate::trigger::TriggerPattern;
use crate::value::Condition;

/// The translator's name of the tokens a creation makes.
const TOKEN: &str = "token";

/// The head of a trigger at the upkeep of the controller of the enchanted creature.
const UPKEEP: &str = "At the beginning of the upkeep of enchanted creature's controller, ";

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
        Value::Array(items) => {
            let mut found = false;
            for item in items {
                found |= rename(item);
            }
            found
        }
        Value::Object(map) => {
            let mut found = false;
            if map.contains_key("token") && map.get("as").and_then(Value::as_str) == Some(TOKEN) {
                map.insert("as".to_owned(), Value::String(TOKENS.to_owned()));
                found = true;
            }
            if map.len() == 1 && map.get("Bound").and_then(Value::as_str) == Some(TOKEN) {
                map.insert("Bound".to_owned(), Value::String(TOKENS.to_owned()));
            }
            for item in map.values_mut() {
                found |= rename(item);
            }
            found
        }
        _ => false,
    }
}

/// How many times the value uses a string.
fn count(value: &Value, wanted: &str) -> usize {
    match value {
        Value::String(text) => usize::from(text == wanted),
        Value::Array(items) => items.iter().map(|item| count(item, wanted)).sum(),
        Value::Object(map) => map.values().map(|item| count(item, wanted)).sum(),
        _ => 0,
    }
}

/// The ability with its created tokens bound as the writers name them, when the one use of the binding attaches the
/// card to the token; `None` when nothing is renamed or the writers' name is already in use.
pub(super) fn wf24_tokens_renamed(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let attach = serde_json::json!({"Attach": {"what": "This", "to": {"Bound": TOKEN}}});
    let text = value.to_string();
    if count(&value, TOKEN) != 2 || !text.contains(&attach.to_string()) {
        return None;
    }
    if uses(&value, TOKENS) || !rename(&mut value) {
        return None;
    }
    serde_json::from_value(value).ok()
}

/// "Whenever a Ninja you control enters, you may attach ~ to it": the card attached to the one object the event is about,
/// in the last-resort pass.
pub(super) fn wf24_attach_to_event(effect: &Effect) -> Option<String> {
    let Effect::Attach {
        what: Ref::This,
        to: Ref::Triggering,
        bind: None,
    } = effect
    else {
        return None;
    };
    super::written_memo::last_resort().then(|| "attach ~ to it".to_owned())
}

/// Whether the last-resort pass may write the ability: a target tapped and given counters ("tap target creature an
/// opponent controls and put a stun counter on it"), or the card attached to the object of the event.
pub(super) fn wf24_last_resort_shape(ability: &Ability) -> bool {
    let Ok(value) = serde_json::to_value(ability) else {
        return false;
    };
    let Some(effects) = value
        .as_object()
        .and_then(|map| map.values().next())
        .and_then(|body| body.get("effects"))
    else {
        return false;
    };
    let attach = serde_json::json!({"Attach": {"what": "This", "to": "Triggering"}});
    // Targets chosen for each player are "each of those creatures", which this pass does not write.
    let objects = value
        .as_object()
        .and_then(|map| map.values().next())
        .and_then(|body| body.pointer("/targets/0/what/Object"))
        .is_some();
    let stunned = objects
        && effects.as_array().is_some_and(|list| {
            list.len() == 2
                && list[0] == serde_json::json!({"Tap": {"what": {"Target": 0}}})
                && list[1]
                    .get("PutCounters")
                    .and_then(|put| put.get("on"))
                    .is_some_and(|on| *on == serde_json::json!({"Target": 0}))
        });
    let attached = *effects == serde_json::json!([attach])
        || *effects == serde_json::json!([{"May": {"effects": [attach]}}]);
    stunned || attached
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
            .filter(|text| {
                text.split_once(" enters, ")
                    .is_some_and(|(head, _)| !head.contains('~'))
                    && text.ends_with(" attach ~ to it.")
            })
            .map(|text| text.replacen(" attach ~ to it.", " attach ~ to that creature.", 1)),
    );
    // "You may have each opponent lose 1 life": a choice of yours about what other players do.
    joined.extend(
        texts
            .iter()
            .filter(|text| text.contains(", you may each other player loses "))
            .map(|text| {
                text.replacen(
                    ", you may each other player loses ",
                    ", you may have each opponent lose ",
                    1,
                )
            }),
    );
    // "Whenever ~ attacks alone, double its power and toughness until end of turn."
    joined.extend(
        texts
            .iter()
            .filter(|text| text.starts_with("Whenever ~ attacks") && text.contains(", double ~'s "))
            .map(|text| text.replacen(", double ~'s ", ", double its ", 1)),
    );
    // "At the beginning of the upkeep of enchanted creature's controller, put a -1/-1 counter on that creature."
    joined.extend(texts.iter().filter_map(|text| {
        let rest = text.strip_prefix(UPKEEP)?;
        rest.contains(" on enchanted creature").then(|| {
            format!(
                "{UPKEEP}{}",
                rest.replace(" on enchanted creature", " on that creature")
            )
        })
    }));
    joined.extend(texts.iter().filter_map(|text| counters_listed(text)));
    for text in joined {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
