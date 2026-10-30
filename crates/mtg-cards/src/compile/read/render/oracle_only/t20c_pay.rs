//! A payment of generic mana that a stat sets (checker only): the `Pay` of `{1}` repeated `times` is the payment
//! "unless that player pays {X}, where X is this creature's power".

use serde_json::{Value, json};

use super::w4d_parts::{parts, with_effects, wordings};
use crate::ability::Ability;

/// Adds "S unless that player pays {X}, where X is Y." for a payment of `{1}` made `times` times with nothing paid.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let [step] = effects.as_slice() else {
        return;
    };
    let Some(pay) = step.get("Pay").and_then(Value::as_object) else {
        return;
    };
    let one = pay.get("cost") == Some(&json!("{1}"));
    let nothing_paid = pay.get("paid").is_none_or(|paid| paid == &json!([]));
    let Some(times) = pay.get("times").filter(|_| one && nothing_paid) else {
        return;
    };
    let Some(equal) = counted(times) else {
        return;
    };
    let mut plain = pay.clone();
    plain.remove("times");
    plain.insert("cost".to_owned(), json!("{X}"));
    for text in with_effects(&value, &[json!({ "Pay": plain })]) {
        if let Some(sentence) = text.strip_suffix('.')
            && sentence.ends_with(" pays {X}")
        {
            texts.push(format!("{sentence}, where X is {equal}."));
        }
    }
}

/// The words of an amount after "equal to": the wording of a gain of life of the amount, which the writers know.
fn counted(amount: &Value) -> Option<String> {
    let gain = json!({"Spell": {"effects": [{"GainLife": {"amount": amount}}]}});
    wordings(gain).into_iter().find_map(|text| {
        let equal = text.strip_prefix("You gain life equal to ")?;
        equal.strip_suffix('.').map(str::to_owned)
    })
}
