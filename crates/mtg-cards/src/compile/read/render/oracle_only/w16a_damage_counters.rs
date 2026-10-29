//! Damage to the card turned into counters on it: "If damage would be dealt to this creature, put that many +1/+1
//! counters on it instead." The replacement is written as the prevention of that damage and the step is swapped in.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of a replacement of damage to the card by as many counters of one kind on it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(inner) = value.get_mut("Replacement").and_then(Value::as_object_mut) else {
        return;
    };
    let to_this = inner
        .get("when")
        .and_then(|when| when.get("Damage"))
        .and_then(|damage| damage.get("to"))
        == Some(&json!("This"));
    let Some(counter) = inner.get("by").and_then(w16a_counters_instead) else {
        return;
    };
    if !to_this {
        return;
    }
    inner.insert("by".to_owned(), json!("Prevent"));
    let step = format!("put that many {counter} counters on it instead");
    for text in wordings(value) {
        if let Some(body) = text.strip_suffix(", prevent that damage.") {
            texts.push(format!("{body}, {step}."));
        }
    }
}

/// The counter of `{"Instead": [{"PutCounters": {"on": "This", "count": "Triggering"}}]}`.
fn w16a_counters_instead(by: &Value) -> Option<String> {
    let [step] = by.get("Instead")?.as_array()?.as_slice() else {
        return None;
    };
    let put = step.get("PutCounters")?;
    if put.get("on")? != "This" || put.get("count")? != "Triggering" {
        return None;
    }
    put.get("counter")?.as_str().map(str::to_owned)
}
