//! Damage a creature deals equal to its own power ("it deals damage equal to its power to target player or
//! planeswalker"): one `DealDamage` whose amount is the power of its own source, worded after the renderer's damage of
//! one with the amount moved after the noun.

use super::super::alternatives;
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use serde_json::{Value, json};

/// Adds the wordings of an ability whose one effect is damage equal to the power of its own source.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(Value::Array(effects)) = serde_json::to_value(ability.effects()) else {
        return;
    };
    let [effect] = effects.as_slice() else {
        return;
    };
    for sentence in c10_cen_power_damage(ability, effect) {
        texts.extend(with_effects(ability, &sentence));
    }
}

/// The sentences of the damage with "deals 1 damage to" read as "deals damage equal to its power to", and the same
/// with "it" as the subject.
fn c10_cen_power_damage(ability: &Ability, effect: &Value) -> Vec<String> {
    let Some(damage) = effect.get("DealDamage") else {
        return Vec::new();
    };
    let source = damage.get("source");
    let power = json!({"Stat": {"of": source, "stat": "Power"}});
    if source.is_none() || damage.get("amount") != Some(&power) {
        return Vec::new();
    }
    let subjects: &[&str] = match source.and_then(Value::as_str) {
        Some("This") => &["~", "this creature"],
        Some("Attached") => &["it", "equipped creature", "enchanted creature"],
        Some("Triggering") => &["it", "that creature"],
        _ => return Vec::new(),
    };
    let Some(to) = damage.get("to") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for rest in c10_cen_recipients(ability, to) {
        for subject in subjects {
            out.push(format!(
                "{subject} deals damage equal to its power to {rest}"
            ));
        }
    }
    out
}

/// The wordings of the damage's recipient: the renderer's for damage of one without a source, else the player of the
/// event or the attacked player or planeswalker.
fn c10_cen_recipients(ability: &Ability, to: &Value) -> Vec<String> {
    if to == &json!({"ControllerOf": "Triggering"}) {
        return vec!["that player".to_owned()];
    }
    if to == "Attacked" {
        return vec!["the player or planeswalker it's attacking".to_owned()];
    }
    let Ok(plain) =
        serde_json::from_value::<Effect>(json!({"DealDamage": {"to": to, "amount": 1}}))
    else {
        return Vec::new();
    };
    alternatives(ability.targets(), &[plain])
        .into_iter()
        .filter_map(|sentences| match sentences.as_slice() {
            [one] => one
                .split_once(" deals 1 damage to ")
                .map(|(_, rest)| rest.to_owned()),
            _ => None,
        })
        .collect()
}
