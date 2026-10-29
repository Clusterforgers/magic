//! Protection from a group of sources the keyword words do not name: "protection from non-Human creatures", "from
//! Dragons and Wizards"-style groups of a type or a creature type. The ability is written with protection from red in
//! its place and the words are swapped.

use super::super::super::selectors::{Form, granted_text, phrase_text};
use super::w4b_grants::replace;
use super::w4b_stand::{json, stand, swapped};
use crate::ability::Ability;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Selector};
use serde_json::{Value, json};

/// Adds the wordings of an ability that gives protection from one group of sources.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let value = json(ability);
    let mut found = Vec::new();
    protections(&value, &mut found);
    let [(grant, words)] = found.as_slice() else {
        return;
    };
    let stand_in = json!({"AddKeyword": {"Protection": [{"Color": "Red"}]}});
    // A stand-in the ability has already would swap the wrong words.
    if replace(&mut value.clone(), &stand_in, &stand_in).is_some() {
        return;
    }
    let Some(written) = stand(ability, |value| replace(value, grant, &stand_in)) else {
        return;
    };
    let to = format!("protection from {words}");
    texts.extend(swapped(&written, &[("protection from red", to.as_str())]));
}

/// Each protection in the JSON from sources of a group the keyword words do not name, with its words.
fn protections(value: &Value, found: &mut Vec<(Value, String)>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Array(preds)) = map.get("Protection")
                && let Some(words) = group_words(preds)
            {
                found.push((json!({"AddKeyword": {"Protection": preds}}), words));
            }
            map.values().for_each(|inner| protections(inner, found));
        }
        Value::Array(items) => items.iter().for_each(|inner| protections(inner, found)),
        _ => {}
    }
}

/// The words of a group of sources of a type or a creature type, when the keyword has none of its own.
fn group_words(preds: &[Value]) -> Option<String> {
    let selector = Selector(
        preds
            .iter()
            .map(|pred| serde_json::from_value::<Pred>(pred.clone()))
            .collect::<Result<_, _>>()
            .ok()?,
    );
    let named = selector
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::Type(_) | Pred::Subtype(_)));
    let given = [Modification::AddKeyword(Keyword::Protection(
        selector.clone(),
    ))];
    (named && granted_text(&given).is_none()).then_some(())?;
    phrase_text(&selector, Form::All)
}
