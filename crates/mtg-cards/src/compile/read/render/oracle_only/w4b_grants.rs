//! What a static ability or a continuous effect gives that the renderers do not word: a keyword the keyword list does
//! not read ("Other creatures you control have dethrone."), protection from each of your opponents, and "can't be
//! blocked except by Spirits". The ability is written with flying, protection from red or "can't be blocked" in its
//! place, and the words of the stand-in are swapped for the grant's.

use super::super::super::selectors::keyword_text;
use super::w4b_stand::{json, stand, swapped};
use crate::ability::Ability;
use crate::keyword::Keyword;
use serde_json::{Value, json};

/// Adds the wordings of an ability that gives one of the grants.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let value = json(ability);
    let mut mods = Vec::new();
    collect_mods(&value, &mut mods);
    for found in mods {
        if let Some((stand_in, from, to)) = grant(&found) {
            // A stand-in the ability has already would swap the wrong words.
            if replace(&mut value.clone(), &stand_in, &stand_in).is_some() {
                continue;
            }
            let Some(written) = stand(ability, |value| replace(value, &found, &stand_in)) else {
                continue;
            };
            for (from, to) in [(from.clone(), to.clone()), (capital(&from), capital(&to))] {
                texts.extend(swapped(&written, &[(from.as_str(), to.as_str())]));
            }
        }
    }
}

/// The stand-in of a grant and the words to swap: the stand-in's words and the grant's.
fn grant(modification: &Value) -> Option<(Value, String, String)> {
    let protection = json!({"AddKeyword": {"Protection": [{"ControlledBy": "Opponents"}]}});
    if *modification == protection {
        return Some((
            json!({"AddKeyword": {"Protection": [{"Color": "Red"}]}}),
            "protection from red".to_owned(),
            "protection from each of your opponents".to_owned(),
        ));
    }
    if let Some(name) = modification.pointer("/AddKeyword").and_then(Value::as_str) {
        let keyword: Keyword = serde_json::from_value(json!(name)).ok()?;
        keyword_text(&keyword).is_none().then_some(())?;
        return Some((
            json!({"AddKeyword": "Flying"}),
            "flying".to_owned(),
            keyword.name().to_lowercase(),
        ));
    }
    let blockers = modification.pointer("/CantBeBlockedBy")?;
    let [only] = blockers.as_array()?.as_slice() else {
        return None;
    };
    let subtype = only.pointer("/Not/Subtype")?.as_str()?;
    Some((
        json!("CantBeBlocked"),
        "can't be blocked this turn".to_owned(),
        format!("can't be blocked this turn except by {subtype}s"),
    ))
}

/// The modifications of every static ability and continuous effect in the JSON.
fn collect_mods(value: &Value, found: &mut Vec<Value>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if key == "mods" {
                    found.extend(inner.as_array().into_iter().flatten().cloned());
                } else {
                    collect_mods(inner, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_mods(item, found)),
        _ => {}
    }
}

/// Replaces the first `from` in the JSON with `to`; `None` when there is none.
pub(super) fn replace(value: &mut Value, from: &Value, to: &Value) -> Option<()> {
    if value == from {
        *value = to.clone();
        return Some(());
    }
    match value {
        Value::Object(map) => map.values_mut().find_map(|inner| replace(inner, from, to)),
        Value::Array(items) => items.iter_mut().find_map(|inner| replace(inner, from, to)),
        _ => None,
    }
}

/// The text with its first letter in upper case.
fn capital(text: &str) -> String {
    super::super::capitalize(text)
}
