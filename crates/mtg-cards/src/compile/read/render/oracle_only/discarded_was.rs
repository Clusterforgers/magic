//! A step that depends on the kind of the card the cost discarded: "{T}, Discard a card: Draw a card. If the discarded card
//! was a creature card, amass Orcs 1."

use super::super::variants;
use super::twin_json::{lowered, twin};
use crate::ability::Ability;
use serde_json::Value;

/// The card type the step asks for and the ability with the step's effect in the list or out of it.
fn parts(ability: &Ability, keep: bool) -> Option<(Ability, String)> {
    let mut kind = None;
    let plain = twin(ability, |value| {
        let activated = value.get_mut("Activated")?;
        let cost = serde_json::to_string(activated.get("cost")?).ok()?;
        cost.contains("\"Discard\"").then_some(())?;
        let effects = activated.get_mut("effects")?.as_array_mut()?;
        let [_, step] = effects.as_mut_slice() else {
            return None;
        };
        let step = step.get_mut("If")?;
        let matches = step.get("condition")?.get("Matches")?;
        (matches.get("subject")? == "Paid").then_some(())?;
        let selector = matches.get("selector")?.as_array()?;
        kind = match selector.as_slice() {
            [one] => one.get("Type")?.as_str().map(str::to_lowercase),
            _ => None,
        };
        let then = step.get("then")?.clone();
        if keep {
            let Value::Array(then) = then else {
                return None;
            };
            let first = effects.remove(0);
            *effects = std::iter::once(first).chain(then).collect();
        } else {
            effects.pop();
        }
        Some(())
    })?;
    Some((plain, kind?))
}

/// Adds the wordings of an ability whose second step needs the discarded card to be of a type.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let (Some((short, kind)), Some((full, _))) = (parts(ability, false), parts(ability, true))
    else {
        return;
    };
    let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    let longer = variants(&full);
    let mut said = Vec::new();
    for opening in variants(&short) {
        for text in &longer {
            if let Some(step) = text.strip_prefix(&opening) {
                said.push(format!(
                    "{opening} If the discarded card was {article} {kind} card, {}",
                    lowered(step.trim())
                ));
            }
        }
    }
    texts.extend(said);
}
