//! Two different targets destroyed in one sentence: "Destroy target artifact and target enchantment." The renderers
//! write a sentence for each target.

use crate::ability::Ability;
use crate::effect::{Effect, Mode};
use crate::refs::Ref;

/// Whether the effects destroy the first target and then the second.
fn destroys_both(effects: &[Effect]) -> bool {
    matches!(
        effects,
        [
            Effect::Destroy { what: Ref::Target(0), .. },
            Effect::Destroy { what: Ref::Target(1), .. },
        ]
    )
}

/// Adds the wordings that join the two destroyed targets with "and", also in the mode of a modal spell.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let in_mode = effects.iter().any(|effect| {
        matches!(effect, Effect::Modal { modes, .. }
            if modes.iter().any(|Mode { effects, .. }| destroys_both(effects)))
    });
    if !destroys_both(effects) && !in_mode {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(". Then destroy target "))
        .map(|text| text.replacen(". Then destroy target ", " and target ", 1))
        .collect();
    texts.extend(said);
}
