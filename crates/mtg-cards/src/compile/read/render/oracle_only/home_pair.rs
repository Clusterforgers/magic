//! Two targets that return to their owners' hands in one sentence: "Return target creature you control and target
//! creature you don't control to their owners' hands." The renderers write a sentence for each target.

use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::Ref;

/// Adds the wording of a spell that returns its first and second target to their owners' hands.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let [
        Effect::Move {
            what: Ref::Target(0),
            to: Destination::Hand,
        },
        Effect::Move {
            what: Ref::Target(1),
            to: Destination::Hand,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    if targets.len() != 2 {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| paired(text)).collect();
    texts.extend(found);
}

/// "Return target A to its owner's hand. Return target B to its owner's hand." as one sentence.
fn paired(text: &str) -> Option<String> {
    let (first, second) = text
        .split_once(". Return target ")
        .or_else(|| text.split_once(". Then return target "))?;
    let a = first
        .strip_prefix("Return target ")?
        .strip_suffix(" to its owner's hand")?;
    let b = second.strip_suffix(" to its owner's hand.")?;
    Some(format!(
        "Return target {a} and target {b} to their owners' hands."
    ))
}
