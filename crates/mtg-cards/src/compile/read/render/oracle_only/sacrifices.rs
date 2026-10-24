//! A sacrifice of the player's own without "you": "Sacrifice a land." opens a sentence, "..., then sacrifice a permanent"
//! follows another step of the player's and "..., sacrifice a creature" follows an event.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref};

/// Adds the wordings that leave out "you" before a sacrifice the controller chooses.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !ability.effects().iter().any(chosen) {
        return;
    }
    let found: Vec<String> = texts.iter().flat_map(|text| imperative(text)).collect();
    texts.extend(found);
}

/// A sacrifice by the controller of something other than the card itself.
fn chosen(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::Sacrifice { who: Ref::You, filter, .. } if filter.0 != [Pred::This]
    )
}

/// The text with the first "you sacrifice" worded as the verb alone, in each place the oracle puts it.
fn imperative(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(rest) = text.strip_prefix("You sacrifice ") {
        found.push(format!("Sacrifice {rest}"));
    }
    for (said, wording) in [
        (". You sacrifice ", ". Sacrifice "),
        (". You sacrifice ", ", then sacrifice "),
        (". Then you sacrifice ", ". Then sacrifice "),
        (", you sacrifice ", ", sacrifice "),
    ] {
        if text.contains(said) {
            found.push(text.replacen(said, wording, 1));
        }
    }
    found
}
