//! A later step on the group an earlier step named: "Creatures you control get +2/+1 until end of turn. Untap those
//! creatures." The renderers name the group again; the step is written on the source and `~` becomes "those creatures".

use serde_json::{Value, json};

use super::w4d_parts::{parts, with_effects};
use crate::ability::Ability;

/// The keys of an effect that hold the object or group it acts on.
const W13A_GROUP_KEYS: [&str; 3] = ["affected", "what", "on"];

/// Adds the wordings of an ability whose last effect acts on the group of creatures the effect before it named.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, mut effects)) = parts(ability) else {
        return;
    };
    let [.., before, last] = effects.as_mut_slice() else {
        return;
    };
    let Some(group) = w13a_group(before).cloned() else {
        return;
    };
    let creatures = group
        .get("Each")
        .and_then(Value::as_array)
        .is_some_and(|preds| preds.contains(&json!({"Type": "Creature"})));
    let Some(what) = w13a_inner(last).and_then(|inner| inner.get_mut("what")) else {
        return;
    };
    if !creatures || *what != group {
        return;
    }
    *what = json!("This");
    for text in with_effects(&value, &effects) {
        let Some((head, tail)) = text.rsplit_once(". ") else {
            continue;
        };
        if !head.contains('~') && tail.matches('~').count() == 1 {
            texts.push(format!("{head}. {}", tail.replace('~', "those creatures")));
        }
    }
}

/// The inner object of an effect: `{"Untap": {...}}` gives the `{...}`.
fn w13a_inner(effect: &mut Value) -> Option<&mut serde_json::Map<String, Value>> {
    effect.as_object_mut()?.values_mut().next()?.as_object_mut()
}

/// The group an effect acts on, when it is a selector of each object.
fn w13a_group(effect: &Value) -> Option<&Value> {
    let inner = effect.as_object()?.values().next()?;
    W13A_GROUP_KEYS
        .iter()
        .find_map(|key| inner.get(*key))
        .filter(|group| group.get("Each").is_some())
}
