//! l29-3 events and their results: "whenever you clash" (`Action::Clashed`, won or lost) with "If you won, S." after it,
//! read off the amount of the clash (1 when the player won), and its writers.

use serde_json::Value;

use super::tokens::Reader;
use crate::ability::Ability;
use crate::did::Action;
use crate::refs::{Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition};

/// The condition "you won" after a clash: the amount of the clash event, 1 when the player who clashed won.
#[must_use]
pub(super) fn l293_won() -> Condition {
    Condition::Compare {
        left: Amount::Triggering,
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    }
}

/// "you clash" (any result), "you clash and win" (`W15cClashWon`), after the player and the verb.
pub(super) fn l293_clashed(r: &mut Reader<'_>, by: Ref) -> TriggerPattern {
    if r.words("and win") || r.words("and wins") {
        return TriggerPattern::W15cClashWon { who: by };
    }
    TriggerPattern::Did {
        action: Action::Clashed,
        who: by,
        what: Selector::default(),
    }
}

/// The words of a player who clashes: "you clash", "an opponent clashes", "a player clashes".
pub(super) fn l293_player_action(action: Action, who: &Ref) -> Option<String> {
    let Action::Clashed = action else {
        return None;
    };
    Some(
        match who {
            Ref::You => "you clash",
            Ref::Opponents => "an opponent clashes",
            Ref::EachPlayer => "a player clashes",
            _ => return None,
        }
        .to_owned(),
    )
}

/// "you won": the clash of the trigger was won, read only as the condition of a sentence.
pub(super) fn l293_won_condition(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("you won")?;
    r.at_punct(',').then(l293_won)
}

/// The words of the condition after "if" when it is the result of the clash.
pub(super) fn l293_condition_text(condition: &Condition) -> Option<String> {
    (*condition == l293_won()).then(|| "you won".to_owned())
}

/// Whether the pattern is a clash by one player, won or lost.
fn clashes(pattern: &TriggerPattern) -> bool {
    matches!(
        pattern,
        TriggerPattern::Did {
            action: Action::Clashed,
            what,
            ..
        } if what.is_empty()
    )
}

/// Keeps "If you won" only in a trigger on a clash, the one event whose amount is its result.
pub(super) fn l293_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let on_clash = matches!(ability, Ability::Triggered { when, .. } if clashes(when));
    if !on_clash {
        texts.retain(|text| !text.contains("you won"));
    }
}

/// The sentence that opens the result of the clash; the card ends with it.
const WON: &str = ". If you won, ";

/// Whether the JSON value is the `If` of a won clash with no "otherwise".
fn won_if(value: &Value) -> Option<&Value> {
    let body = value.get("If")?.as_object()?;
    let won = serde_json::to_value(l293_won()).ok()?;
    (body.get("condition") == Some(&won) && !body.contains_key("otherwise"))
        .then(|| body.get("then"))
        .flatten()
}

/// The ability with the `If` of a won clash that ends an effect list replaced by its effects (`inline`) or left out.
fn without_won(value: &Value, inline: bool) -> Option<Value> {
    match value {
        Value::Array(items) => {
            if let Some(then) = items.last().and_then(won_if) {
                let mut items = items[..items.len() - 1].to_vec();
                if inline {
                    items.extend(then.as_array()?.iter().cloned());
                }
                return Some(Value::Array(items));
            }
            let mut found = None;
            let changed: Vec<Value> = items
                .iter()
                .map(|item| match (found.is_none(), without_won(item, inline)) {
                    (true, Some(new)) => {
                        found = Some(());
                        new
                    }
                    _ => item.clone(),
                })
                .collect();
            found.map(|()| Value::Array(changed))
        }
        Value::Object(map) => {
            let mut found = None;
            let changed: serde_json::Map<String, Value> = map
                .iter()
                .map(
                    |(key, item)| match (found.is_none(), without_won(item, inline)) {
                        (true, Some(new)) => {
                            found = Some(());
                            (key.clone(), new)
                        }
                        _ => (key.clone(), item.clone()),
                    },
                )
                .collect();
            found.map(|()| Value::Object(changed))
        }
        _ => None,
    }
}

/// The text with its first letter in lower case.
fn lowered(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// The text with its first letter in upper case.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// "S. If you won, T.": the clash trigger's last sentence written from the twin that does T unconditionally, after the
/// wording of the twin without it.
pub(super) fn l293_won_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let on_clash = matches!(ability, Ability::Triggered { when, .. } if clashes(when));
    let Some(value) = on_clash
        .then(|| serde_json::to_value(ability).ok())
        .flatten()
    else {
        return;
    };
    let twin = |inline: bool| {
        without_won(&value, inline)
            .map(|mut value| {
                let whole = value.to_string();
                unread_names_dropped(&mut value, &whole);
                value
            })
            .and_then(|value| serde_json::from_value::<Ability>(value).ok())
            .map(|ability| written(&ability))
            .unwrap_or_default()
    };
    let (whole, heads) = (twin(true), twin(false));
    for head in heads {
        let head = head.trim_end_matches('.');
        for text in &whole {
            let Some(rest) = text
                .strip_prefix(head)
                .and_then(|rest| rest.strip_prefix(". "))
                .filter(|rest| !rest.starts_with("Then "))
            else {
                continue;
            };
            let won = format!("{head}{WON}{}", lowered(rest));
            if !texts.contains(&won) {
                texts.push(won);
            }
        }
    }
}

/// The ability `inline` with the effects it has beyond `head` in the one list where it has more put under the `If` of a
/// won clash.
fn wrapped(inline: &Value, head: &Value) -> Option<Value> {
    match (inline, head) {
        (Value::Array(more), Value::Array(fewer)) => {
            if more.len() > fewer.len() && more[..fewer.len()] == fewer[..] {
                let then = Value::Array(more[fewer.len()..].to_vec());
                let mut items = fewer.clone();
                items.push(serde_json::json!({ "If": { "condition": serde_json::to_value(l293_won()).ok()?, "then": then } }));
                return Some(Value::Array(items));
            }
            (more.len() == fewer.len()).then_some(())?;
            let at = more.iter().zip(fewer).position(|(a, b)| a != b)?;
            let mut items = more.clone();
            items[at] = wrapped(&more[at], &fewer[at])?;
            Some(Value::Array(items))
        }
        (Value::Object(more), Value::Object(fewer)) => {
            (more.len() == fewer.len()).then_some(())?;
            let key = more.keys().find(|key| more.get(*key) != fewer.get(*key))?;
            let mut map = more.clone();
            map.insert(key.clone(), wrapped(more.get(key)?, fewer.get(key)?)?);
            Some(Value::Object(map))
        }
        _ => None,
    }
}

/// "Whenever you clash, S. If you won, T.": read from the twin that does T unconditionally and the one without it.
pub(super) fn l293_won_read(text: &str, read: impl Fn(&str) -> Option<Ability>) -> Option<Ability> {
    let (head, tail) = text.split_once(WON)?;
    (!tail.contains(". ")).then_some(())?;
    let inline = serde_json::to_value(read(&format!("{head}. {}", capitalized(tail)))?).ok()?;
    let head = serde_json::to_value(read(&format!("{head}."))?).ok()?;
    let found: Ability = serde_json::from_value(wrapped(&inline, &head)?).ok()?;
    matches!(&found, Ability::Triggered { when, .. } if clashes(when)).then_some(found)
}

/// The value without the names (`as`) nothing in `whole` reads any more.
fn unread_names_dropped(value: &mut Value, whole: &str) {
    match value {
        Value::Array(items) => {
            for item in items {
                unread_names_dropped(item, whole);
            }
        }
        Value::Object(map) => {
            let unread = map
                .get("as")
                .and_then(Value::as_str)
                .is_some_and(|name| !whole.contains(&format!("{{\"Bound\":\"{name}\"}}")));
            if unread {
                map.remove("as");
            }
            for item in map.values_mut() {
                unread_names_dropped(item, whole);
            }
        }
        _ => {}
    }
}
