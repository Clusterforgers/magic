//! An entry with counters under a condition: "This creature enters with a +1/+1 counter on it if you attacked this
//! turn." The replacement is written without its condition and the condition follows as "if ...".

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;
use crate::compile::read::conditional::condition_texts;
use crate::value::Condition;

/// Adds the wordings of an entry with counters whose replacement has a condition.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(inner) = value.get_mut("Replacement").and_then(Value::as_object_mut) else {
        return;
    };
    if inner
        .get("by")
        .and_then(|by| by.get("WithCounters"))
        .is_none()
    {
        return;
    }
    let Some(condition) = inner.remove("condition").filter(|c| !c.is_null()) else {
        return;
    };
    let facts = w16a_condition_words(&condition);
    for text in wordings(value) {
        let Some(body) = text.strip_suffix('.').filter(|body| !body.contains('.')) else {
            continue;
        };
        texts.extend(facts.iter().map(|fact| format!("{body} if {fact}.")));
    }
}

/// The wordings of a condition after "if"; a count greater than a number is also said as at least the next number.
pub(super) fn w16a_condition_words(condition: &Value) -> Vec<String> {
    let mut found = Vec::new();
    for form in [condition.clone(), w16a_at_least(condition)] {
        if let Ok(condition) = serde_json::from_value::<Condition>(form) {
            found.extend(condition_texts(&condition));
        }
    }
    found.dedup();
    found
}

/// The condition with a comparison "greater than N" of its top level turned into "at least N + 1".
fn w16a_at_least(condition: &Value) -> Value {
    let mut condition = condition.clone();
    if let Some(compare) = condition.get_mut("Compare").and_then(Value::as_object_mut)
        && compare.get("op") == Some(&json!("Gt"))
        && let Some(number) = compare.get("right").and_then(Value::as_u64)
    {
        compare.insert("op".to_owned(), json!("Ge"));
        compare.insert("right".to_owned(), json!(number + 1));
    }
    condition
}
