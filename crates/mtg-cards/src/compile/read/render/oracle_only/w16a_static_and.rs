//! Two parts of a static ability joined by "and": "Enchanted creature gets +2/+2 and can't attack." The first
//! modification and the others are written apart and joined where they name the same subject.

use serde_json::Value;

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// The verbs a part of a static ability opens with after its subject.
const W16A_VERBS: [&str; 6] = [" gets ", " get ", " has ", " can't ", " is ", " doesn't "];

/// Adds the wordings of a static ability whose first modification and the rest are joined by "and".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(mods) = value
        .get("Static")
        .and_then(|inner| inner.get("mods"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let Some((first, rest)) = mods.split_first().filter(|(_, rest)| !rest.is_empty()) else {
        return;
    };
    let firsts = w16a_with_mods(&value, std::slice::from_ref(first));
    let rests = w16a_with_mods(&value, rest);
    for one in &firsts {
        let Some(subject) = w16a_subject(one) else {
            continue;
        };
        let Some(head) = one.strip_suffix('.') else {
            continue;
        };
        for other in &rests {
            if let Some(tail) = other
                .strip_prefix(subject)
                .and_then(|tail| tail.strip_prefix(' '))
            {
                texts.push(format!("{head} and {tail}"));
            }
        }
    }
}

/// The wordings of the static ability with these modifications in place of its own.
fn w16a_with_mods(value: &Value, mods: &[Value]) -> Vec<String> {
    let mut value = value.clone();
    if let Some(inner) = value.get_mut("Static").and_then(Value::as_object_mut) {
        inner.insert("mods".to_owned(), Value::Array(mods.to_vec()));
    }
    wordings(value)
        .into_iter()
        .filter(|text| text.ends_with('.') && text.matches('.').count() == 1)
        .collect()
}

/// The subject of a one-sentence wording: the words before its first verb.
fn w16a_subject(text: &str) -> Option<&str> {
    let at = W16A_VERBS.iter().filter_map(|verb| text.find(verb)).min()?;
    Some(&text[..at])
}
