//! Trigger events the translator words as one zone change with a compound subject (r6d), written as the readers
//! give them:
//! - "Whenever ~ or another Ally you control enters": the subject `This or not (not Ally or not Other or not yours)`
//!   is either event of `AnyOf`, the card's own and that of the other objects;
//! - "When ~ enters, if it was kicked" and "if you cast it (from your hand)": the card's own entry with a fact of its
//!   cast is the entry with that fact as the intervening "if"; a cast or a kick never changes later.

use serde_json::{Value, json};

use crate::ability::Ability;

/// The ability with its event written the readers' way, or `None` when it has no such event.
pub(super) fn normal(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.get_mut("Triggered")?;
    let event = body.get("when")?.get("ZoneChange")?.clone();
    let [subject] = event.get("what")?.as_array()?.as_slice() else {
        return None;
    };
    let parts = subject.get("AnyOf")?.as_array()?;
    let others = match parts.as_slice() {
        [this, other] | [other, this] if this == "This" => all_of(other)?,
        _ => return None,
    };
    let mut own = event.clone();
    own["what"] = json!(["This"]);
    let mut theirs = event;
    theirs["what"] = Value::Array(others);
    body["when"] = json!({ "AnyOf": [{ "ZoneChange": own }, { "ZoneChange": theirs }] });
    serde_json::from_value(value).ok()
}

/// `not (not a or not b ...)`: the predicates a, b ... that hold together.
fn all_of(value: &Value) -> Option<Vec<Value>> {
    value
        .get("Not")?
        .get("AnyOf")?
        .as_array()?
        .iter()
        .map(|part| part.get("Not").cloned())
        .collect()
}

/// The ability without the fact of its cast and the words of that fact as an intervening "if", or `None`.
fn cast_fact(ability: &Ability) -> Option<(Ability, &'static [&'static str])> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.get_mut("Triggered")?;
    if body.get("intervening_if").is_some() {
        return None;
    }
    let event = body.get_mut("when")?.get_mut("ZoneChange")?;
    if event.get("to")? != "Battlefield" || event.get("from").is_some() {
        return None;
    }
    let what = event.get("what")?.as_array()?;
    if !what.iter().any(|pred| pred == "This") {
        return None;
    }
    let mut rest: Vec<&Value> = what.iter().filter(|pred| *pred != "This").collect();
    rest.sort_by_key(ToString::to_string);
    let words: &'static [&'static str] = match rest.as_slice() {
        [kicked] if *kicked == "Kicked" => &["it was kicked", "he was kicked", "she was kicked"],
        [cast] if **cast == json!({ "WasCast": { "by": "You" } }) => &["you cast it"],
        // A card in your hand is yours: owning it adds nothing to the cast from there.
        [cast] | [cast, _] | [_, cast]
            if **cast == json!({ "WasCast": { "by": "You", "from": "Hand" } })
                && rest
                    .iter()
                    .all(|pred| pred == cast || **pred == json!({ "OwnedBy": "You" })) =>
        {
            &["you cast it from your hand"]
        }
        _ => return None,
    };
    event["what"] = json!(["This"]);
    Some((serde_json::from_value(value).ok()?, words))
}

/// The wordings of a trigger on the card's own entry with a fact of its cast: "When ~ enters, if it was kicked, ...".
pub(super) fn cast_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let Some((plain, words)) = cast_fact(ability) else {
        return Vec::new();
    };
    written(&plain)
        .iter()
        .filter_map(|text| text.split_once(", "))
        .flat_map(|(event, rest)| {
            words
                .iter()
                .map(move |fact| format!("{event}, if {fact}, {rest}"))
        })
        .collect()
}
