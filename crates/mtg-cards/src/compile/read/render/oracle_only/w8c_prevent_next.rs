//! A shield of the next amount of damage to one object or player this turn: "Prevent the next 2 damage that would be
//! dealt to target creature this turn." and, counted as it begins, "Prevent the next X damage that would be dealt to
//! target creature this turn, where X is the number of Clerics on the battlefield." The ability is written with damage of
//! that amount to the same recipient, whose sentence names it, and the sentence is turned into the prevention.

use serde_json::{Value, json};

use super::w4d_parts::{parts, wordings};
use crate::ability::Ability;

/// Adds the wordings of an ability with one shield of the next damage to a recipient this turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((mut value, effects)) = parts(ability) else {
        return;
    };
    let shields: Vec<(usize, Value, Value)> = effects
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
    for text in wordings(value) {
        if let Some(prevented) = w8c_prevented(&text) {
            texts.push(prevented);
        }
    }
}

/// The amount and the recipient of a shield of the next damage this turn from any source.
fn w8c_shield(effect: &Value) -> Option<(Value, Value)> {
    let shield = effect.get("Shield")?;
    let by = shield.get("by")?;
    // A counted amount is written as the damage of that amount is.
    let amount = match by.get("PreventUpTo") {
        Some(number) => json!(number.as_u64()?),
        None => by.get("PreventCounted")?.clone(),
    };
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

/// The wording with its one sentence of damage ("~ deals 2 damage to <recipient>." or "~ deals X damage to
/// <recipient>, where X is <count>.") turned into the prevention of that damage.
fn w8c_prevented(text: &str) -> Option<String> {
    const DEALT: &str = "~ deals ";
    let (before, rest) = text.split_once(DEALT)?;
    if rest.contains(DEALT) {
        return None;
    }
    let (amount, rest) = rest.split_once(" damage to ")?;
    // A number or X: an amount in more words ("damage equal to ...") is not this sentence.
    if amount.contains(' ') {
        return None;
    }
    let (recipient, after) = rest.split_once('.')?;
    let (recipient, counted) = match recipient.split_once(", where X is ") {
        Some((recipient, count)) if amount == "X" => (recipient, format!(", where X is {count}")),
        Some(_) => return None,
        None => (recipient, String::new()),
    };
    let verb = if before.ends_with(", ") {
        "prevent"
    } else {
        "Prevent"
    };
    Some(format!(
        "{before}{verb} the next {amount} damage that would be dealt to {recipient} this turn{counted}.{after}"
    ))
}
