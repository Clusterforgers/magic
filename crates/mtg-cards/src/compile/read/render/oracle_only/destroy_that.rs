//! The destruction of the creature that dealt damage: "Whenever a creature deals combat damage to you, destroy that
//! creature." The renderers have no wording for the source of the damage as the object of a step, so the step is
//! written for the source and the words are changed.

use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings of a trigger that destroys the dealer of the damage.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let mut dealers = effects.iter().filter(|effect| {
        matches!(
            effect,
            Effect::Destroy {
                what: Ref::Dealer,
                ..
            }
        )
    });
    if dealers.next().is_none() || dealers.next().is_some() {
        return;
    }
    let steps: Vec<Effect> = effects
        .iter()
        .map(|effect| match effect {
            Effect::Destroy {
                what: Ref::Dealer,
                regenerate,
            } => Effect::Destroy {
                what: Ref::This,
                regenerate: *regenerate,
            },
            other => other.clone(),
        })
        .collect();
    let Some(plain) = with_effects(ability, steps) else {
        return;
    };
    texts.extend(
        variants(&plain)
            .iter()
            .filter(|text| text.contains("destroy ~"))
            .map(|text| text.replacen("destroy ~", "destroy that creature", 1)),
    );
}
