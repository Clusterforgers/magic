//! Two wordings the translator writes as structure the renderer has no sentence for: a target only a kicked spell
//! chooses ("If this spell was kicked, target player discards two cards", CR 601.2c, 702.33), written as the same
//! spell with an ordinary target; and "For each of those permanents, its controller creates a token", written as the
//! sentences before it and the token sentence for each target.

use serde_json::{Value, json};

use crate::ability::Ability;

/// The bound of a target only a kicked spell chooses: one when kicked, none otherwise.
fn kicked_bound() -> Value {
    json!({"When": {"condition": "Kicked", "otherwise": 0, "then": 1}})
}

/// Whether `value` names the target `index` anywhere inside it.
fn names_target(value: &Value, index: usize) -> bool {
    match value {
        Value::Object(map) => {
            map.get("Target").and_then(Value::as_u64) == Some(index as u64)
                || map.values().any(|inner| names_target(inner, index))
        }
        Value::Array(items) => items.iter().any(|inner| names_target(inner, index)),
        _ => false,
    }
}

/// Whether the effect is "If this spell was kicked, ..." with nothing done otherwise.
fn kicked_only(effect: &Value) -> bool {
    effect
        .get("If")
        .and_then(Value::as_object)
        .is_some_and(|branch| {
            branch.get("condition") == Some(&json!("Kicked"))
                && branch.keys().all(|key| key == "condition" || key == "then")
        })
}

/// The wordings of a spell or ability whose kicked-only targets are named only inside "if this spell was kicked"
/// sentences: those of the same ability with ordinary targets. A kicked-only target named anywhere else, or a bound
/// of another shape, writes nothing; an ability without such a target is left to the other writers (`None`).
pub(super) fn fix16_kicked_variants(
    ability: &Ability,
    variants: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.as_object_mut()?.values_mut().next()?;
    let bound = kicked_bound();
    let targets = body.get_mut("targets")?.as_array_mut()?;
    let mut kicked = Vec::new();
    for (index, spec) in targets.iter_mut().enumerate() {
        let Some(bounds) = spec.get("bounds") else {
            continue;
        };
        let min = bounds.get("min") == Some(&bound);
        let max = bounds.get("max") == Some(&bound);
        if min != max {
            return Some(Vec::new());
        }
        if min {
            kicked.push(index);
            spec.as_object_mut()?.remove("bounds");
        }
    }
    if kicked.is_empty() {
        return None;
    }
    let effects = body.get("effects")?.as_array()?;
    let outside = effects.iter().any(|effect| {
        !kicked_only(effect) && kicked.iter().any(|index| names_target(effect, *index))
    });
    if outside {
        return Some(Vec::new());
    }
    let plain: Ability = serde_json::from_value(value).ok()?;
    // "If this spell was kicked, it deals 5 damage": the spell is "it" once the sentence names the kick.
    let texts = variants(&plain);
    let pronouns: Vec<String> = (texts.iter())
        .filter(|text| text.contains("kicked, ~ "))
        .map(|text| text.replace("kicked, ~ ", "kicked, it "))
        .collect();
    Some(texts.into_iter().chain(pronouns).collect())
}

/// The wordings of "For each of those permanents, its controller creates <token>." closing an ability whose targets
/// are permanents: the last effect makes, together, one token for the controller of each target. A token for anyone
/// else, more than one token, or a loop over anything but the targets is left to the other writers (`None`).
pub(super) fn fix16_each_target_variants(
    ability: &Ability,
    variants: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let value = serde_json::to_value(ability).ok()?;
    let (kind, body) = value.as_object()?.iter().next()?;
    let effects = body.get("effects")?.as_array()?;
    let (last, before) = effects.split_last()?;
    let [repeat] = last.get("Together")?.get("effects")?.as_array()?.as_slice() else {
        return None;
    };
    let repeat = repeat.get("RepeatEach")?;
    let bound = repeat.get("as")?;
    let [create] = repeat.get("effects")?.as_array()?.as_slice() else {
        return None;
    };
    let create = create.get("CreateToken")?.as_object()?;
    let over_targets = repeat.get("over") == Some(&json!({"Target": 0}))
        && repeat.as_object()?.len() == 3
        && create.get("who") == Some(&json!({"ControllerOf": {"Bound": bound}}))
        && create.keys().all(|key| key == "token" || key == "who");
    if !over_targets || before.is_empty() {
        return None;
    }
    let mut first = body.clone();
    first["effects"] = Value::Array(before.to_vec());
    let first: Ability = serde_json::from_value(json!({ kind: first })).ok()?;
    let token: Ability = serde_json::from_value(json!({
        "Spell": {"effects": [{"CreateToken": {"token": create.get("token")?}}]}
    }))
    .ok()?;
    let made: Vec<String> = variants(&token)
        .iter()
        .filter_map(|text| text.strip_prefix("Create "))
        .map(|rest| format!("For each of those permanents, its controller creates {rest}"))
        .collect();
    // Oracle puts "other" before "target" ("destroy up to three other target nonland permanents").
    let heads: Vec<String> = (variants(&first).into_iter())
        .flat_map(|head| {
            let other = head.replace(" target other ", " other target ");
            [head, other]
        })
        .collect();
    let texts = (heads.iter())
        .flat_map(|head| made.iter().map(move |tail| format!("{head} {tail}")))
        .collect();
    Some(texts)
}
