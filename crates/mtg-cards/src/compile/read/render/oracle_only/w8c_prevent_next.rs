//! A shield of the next amount of damage to one object or player this turn: "Prevent the next 2 damage that would be
//! dealt to target creature this turn." The ability is written with damage of that amount to the same recipient, whose
//! sentence names it, and the sentence is turned into the prevention.

use serde_json::{Value, json};

use super::w4d_parts::{parts, wordings};
use crate::ability::Ability;

/// Adds the wordings of an ability with one shield of the next damage to a recipient this turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((mut value, effects)) = parts(ability) else {
        return;
    };
    let shields: Vec<(usize, u64, Value)> = effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| w8c_shield(effect).map(|(n, to)| (index, n, to)))
        .collect();
    let [(index, amount, to)] = shields.as_slice() else {
        return;
    };
    if effects
        .iter()
        .any(|effect| effect.get("DealDamage").is_some())
    {
        return;
    }
    let Some(slot) = super::twin_json::inner_mut(&mut value)
        .and_then(|inner| inner.get_mut("effects"))
        .and_then(|effects| effects.get_mut(*index))
    else {
        return;
    };
    *slot = json!({"DealDamage": {"amount": amount, "to": to}});
    let dealt = format!("~ deals {amount} damage to ");
    for text in wordings(value) {
        if let Some(prevented) = w8c_prevented(&text, &dealt, *amount) {
            texts.push(prevented);
        }
    }
}

/// The amount and the recipient of a shield of the next damage this turn from any source.
fn w8c_shield(effect: &Value) -> Option<(u64, Value)> {
    let shield = effect.get("Shield")?;
    let amount = shield.get("by")?.get("PreventUpTo")?.as_u64()?;
    if shield.get("duration")? != "UntilEndOfTurn" {
        return None;
    }
    let damage = shield.get("when")?.get("Damage")?.as_object()?;
    if damage
        .iter()
        .any(|(key, part)| key != "to" && !part.is_null())
    {
        return None;
    }
    Some((amount, damage.get("to")?.clone()))
}

/// The wording with its one sentence of damage turned into the prevention of that damage.
fn w8c_prevented(text: &str, dealt: &str, amount: u64) -> Option<String> {
    let (before, rest) = text.split_once(dealt)?;
    if rest.contains(dealt) {
        return None;
    }
    let (recipient, after) = rest.split_once('.')?;
    let verb = if before.ends_with(", ") {
        "prevent"
    } else {
        "Prevent"
    };
    Some(format!(
        "{before}{verb} the next {amount} damage that would be dealt to {recipient} this turn.{after}"
    ))
}
