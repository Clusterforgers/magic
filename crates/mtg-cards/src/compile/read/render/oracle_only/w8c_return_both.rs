//! Two objects returned to the hand in one sentence: "Return this creature and target creature you control to their
//! owner's hand." The renderers write a sentence for each return.

use serde_json::{Value, json};

use super::super::variants;
use super::w4d_parts::parts;
use crate::ability::Ability;

/// The second return as the renderers open it.
const W8C_SECOND: [&str; 2] = [" Return ", " Then return "];

/// Adds the wordings of an ability whose only effects return two objects to their owners' hands.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((_, effects)) = parts(ability) else {
        return;
    };
    if effects.len() != 2 || !effects.iter().all(w8c_to_hand) {
        return;
    }
    let said: Vec<String> = variants(ability)
        .iter()
        .filter_map(|text| w8c_joined(text))
        .flatten()
        .collect();
    texts.extend(said);
}

/// Whether the effect returns one object to its owner's hand.
fn w8c_to_hand(effect: &Value) -> bool {
    effect
        .get("Move")
        .is_some_and(|moved| moved.get("to") == Some(&json!("Hand")))
}

/// The two returns of the wording as one sentence, in either order of the objects.
fn w8c_joined(text: &str) -> Option<Vec<String>> {
    let owner = " to its owner's hand.";
    let (first, second) = W8C_SECOND
        .iter()
        .find_map(|opening| text.split_once(&format!("{owner}{opening}")))?;
    let (head, one) = first
        .split_once("Return ")
        .or_else(|| first.split_once("return "))?;
    let other = second.strip_suffix(owner)?;
    if one.contains('.') || other.contains('.') {
        return None;
    }
    let verb = if head.ends_with(", ") {
        "return"
    } else {
        "Return"
    };
    // Two targets keep the order they are chosen in; an object that is no target may come first.
    let both_targets = one.contains("target") && other.contains("target");
    let orders: &[(&str, &str)] = if both_targets {
        &[(one, other)]
    } else {
        &[(one, other), (other, one)]
    };
    Some(
        orders
            .iter()
            .flat_map(|(a, b)| {
                [
                    format!("{head}{verb} {a} and {b} to their owner's hand."),
                    format!("{head}{verb} {a} and {b} to their owners' hands."),
                ]
            })
            .collect(),
    )
}
