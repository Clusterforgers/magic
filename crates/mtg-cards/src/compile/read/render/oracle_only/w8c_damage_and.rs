//! Two amounts of damage from the source in one sentence: "~ deals 4 damage to any target and 2 damage to you.",
//! "~ deals 1 damage to target creature and 1 damage to target player or planeswalker." The renderers write each as a
//! sentence of its own; the second "~ deals" becomes "and".

use serde_json::Value;

use super::super::variants;
use super::w4d_parts::parts;
use crate::ability::Ability;

/// The openings of a second sentence of damage by the source.
const W8C_SECOND: [&str; 2] = [". ~ deals ", ". Then ~ deals "];

/// Adds the wordings of an ability with two damage effects of the source one after the other.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((_, effects)) = parts(ability) else {
        return;
    };
    if !effects
        .windows(2)
        .any(|pair| pair.iter().all(w8c_own_damage))
    {
        return;
    }
    let joined: Vec<String> = variants(ability)
        .iter()
        .filter_map(|text| w8c_joined(text))
        .collect();
    texts.extend(joined);
}

/// Whether the effect is damage dealt by the source of the ability.
fn w8c_own_damage(effect: &Value) -> bool {
    effect
        .get("DealDamage")
        .is_some_and(|damage| damage.get("source").is_none_or(Value::is_null))
}

/// The wording with the one second sentence of damage joined to the first by "and".
fn w8c_joined(text: &str) -> Option<String> {
    W8C_SECOND.iter().find_map(|second| {
        let (first, rest) = text.split_once(second)?;
        if rest.contains(second) || !first.contains(" damage to ") {
            return None;
        }
        Some(format!("{first} and {rest}"))
    })
}
