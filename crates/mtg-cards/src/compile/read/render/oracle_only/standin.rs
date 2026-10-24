//! An ability whose effects a writer words by itself: the ability is written with one draw in place of its targets and
//! effects, and the sentence of that draw is replaced by the writer's.

use super::super::{capitalize, variants};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// The wordings of the ability with `sentence` (no period at its end, opening in lower case) for its effects.
pub(super) fn with_effects(ability: &Ability, sentence: &str) -> Vec<String> {
    let mut stand = ability.clone();
    match &mut stand {
        Ability::Spell { targets, effects }
        | Ability::Activated {
            targets, effects, ..
        }
        | Ability::Triggered {
            targets, effects, ..
        } => {
            targets.clear();
            *effects = vec![Effect::Draw {
                who: Ref::You,
                count: Amount::one(),
            }];
        }
        _ => return Vec::new(),
    }
    variants(&stand)
        .iter()
        .filter_map(|text| {
            if let Some(head) = text.strip_suffix("Draw a card.") {
                Some(format!("{head}{}.", capitalize(sentence)))
            } else {
                text.strip_suffix("draw a card.")
                    .map(|head| format!("{head}{sentence}."))
            }
        })
        .collect()
}
