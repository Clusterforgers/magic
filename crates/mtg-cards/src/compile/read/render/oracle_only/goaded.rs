//! A goad that follows a step on the same creature: "Goad that creature." The renderers write "Then goad it."

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings of a goad of the target of an earlier step as a sentence of its own.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let goads = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Goad {
                what: Ref::Target(0)
            }
        )
    });
    if !goads {
        return;
    }
    let (from, into) = (". Then goad it", ". Goad that creature");
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
