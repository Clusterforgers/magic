//! A destroyed creature that can't be regenerated: "Destroy target creature. A creature destroyed this way can't be
//! regenerated." The renderers say "That creature can't be regenerated".

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Targetable};
use mtg_core::card::CardType;

/// Adds the wording that names the creature by the destruction.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let no_regeneration = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Destroy {
                regenerate: false,
                ..
            }
        )
    });
    let creature = ability.targets().iter().any(|spec| {
        matches!(&spec.what, Targetable::Object(selector)
            if selector.0.contains(&Pred::Type(CardType::Creature)))
    });
    if !(no_regeneration && creature) {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(". That creature can't be regenerated"))
        .map(|text| {
            text.replacen(
                ". That creature can't be regenerated",
                ". A creature destroyed this way can't be regenerated",
                1,
            )
        })
        .collect();
    texts.extend(found);
}
