//! The card returned to its owner's hand as the cost: "Return this enchantment to its owner's hand: Regenerate target
//! creature." The ability is written with the sacrifice of the card as its cost and the words are swapped.

use serde_json::{Value, json};

use super::w4d_parts::wordings;
use crate::ability::Ability;

/// Adds the wordings of an activated ability whose one cost returns the card itself to its owner's hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(cost) = value
        .get_mut("Activated")
        .and_then(|inner| inner.get_mut("cost"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let [part] = cost.as_mut_slice() else {
        return;
    };
    if !part
        .get("ReturnToHand")
        .and_then(|returned| returned.get("filter"))
        .and_then(Value::as_array)
        .is_some_and(|filter| w16a_itself(filter))
    {
        return;
    }
    *part = json!("SacrificeSelf");
    for text in wordings(value) {
        if let Some(rest) = text.strip_prefix("Sacrifice ~: ") {
            texts.push(format!("Return ~ to its owner's hand: {rest}"));
        }
    }
}

/// Whether the filter names the card itself, with at most its card types beside it.
fn w16a_itself(filter: &[Value]) -> bool {
    let this = json!("This");
    filter
        .iter()
        .any(|pred| *pred == this || pred.get("Is") == Some(&this))
        && filter.iter().all(|pred| {
            *pred == this || pred.get("Is") == Some(&this) || pred.get("Type").is_some()
        })
}
