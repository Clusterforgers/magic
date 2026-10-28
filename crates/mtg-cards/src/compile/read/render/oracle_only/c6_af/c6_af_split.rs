//! An ability with two targets and two steps, the first step acting only on the first target and the second only on the
//! second ("put ... counters on up to one other target creature you control .... Return up to one target creature card
//! ... to the battlefield."): the first step is worded with the ability around it and the second as a spell of its own.

use super::{c6_af_json, c6_af_written};
use crate::ability::Ability;
use serde_json::Value;

/// The first target as JSON, which only the first step may name.
const C6_AF_FIRST: &str = r#"{"Target":0}"#;

/// The second target as JSON, which only the second step may name.
const C6_AF_SECOND: &str = r#"{"Target":1}"#;

/// Adds "S1. S2." for an ability of that shape.
pub(super) fn c6_af_add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((head, tail)) = c6_af_halves(ability) else {
        return;
    };
    let heads = c6_af_written(&head);
    let tails = c6_af_written(&tail);
    for first in &heads {
        for second in &tails {
            texts.push(format!("{first} {second}"));
        }
    }
}

/// The ability with only its first target and step, and a spell with only the second, its target renumbered.
fn c6_af_halves(ability: &Ability) -> Option<(Ability, Ability)> {
    let Value::Object(outer) = serde_json::from_str::<Value>(&c6_af_json(ability)).ok()? else {
        return None;
    };
    let (kind, body) = outer.iter().next()?;
    matches!(kind.as_str(), "Triggered" | "Activated" | "Loyalty").then_some(())?;
    let targets = body.get("targets")?.as_array()?;
    let effects = body.get("effects")?.as_array()?;
    let ([first_target, second_target], [first, second]) = (targets.as_slice(), effects.as_slice())
    else {
        return None;
    };
    let names = |step: &Value, own: &str, other: &str| {
        let json = step.to_string();
        json.contains(own) && !json.contains(other)
    };
    (names(first, C6_AF_FIRST, C6_AF_SECOND) && names(second, C6_AF_SECOND, C6_AF_FIRST))
        .then_some(())?;
    let mut head_body = body.clone();
    let head_map = head_body.as_object_mut()?;
    head_map.insert("targets".into(), Value::Array(vec![first_target.clone()]));
    head_map.insert("effects".into(), Value::Array(vec![first.clone()]));
    let head_json = serde_json::json!({ kind: head_body }).to_string();
    let tail_step = second.to_string().replace(C6_AF_SECOND, C6_AF_FIRST);
    let tail_json = format!(
        r#"{{"Spell":{{"targets":[{}],"effects":[{tail_step}]}}}}"#,
        second_target
    );
    Some((
        serde_json::from_str(&head_json).ok()?,
        serde_json::from_str(&tail_json).ok()?,
    ))
}
