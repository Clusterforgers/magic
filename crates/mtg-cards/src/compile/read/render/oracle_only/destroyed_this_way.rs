//! What follows the destruction a player chose to make is worded by the object: "you may destroy another target creature.
//! If a creature is destroyed this way, you gain life equal to its toughness." The renderers write "If you do".

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Targetable};
use crate::value::Condition;
use mtg_core::card::CardType;

/// Adds the wording that names the creature destroyed after an optional destruction of the one target creature.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let may_destroy = ability.effects().windows(2).any(|pair| {
        matches!(
            pair,
            [
                Effect::May { effects, .. },
                Effect::If { condition: Condition::Performed, .. },
            ] if effects.iter().any(|effect| matches!(effect, Effect::Destroy { .. }))
        )
    });
    let creature = matches!(
        ability.targets(),
        [spec] if matches!(&spec.what, Targetable::Object(selector) if selector.0.contains(&Pred::Type(CardType::Creature)))
    );
    if !(may_destroy && creature) {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(". If you do, "))
        .map(|text| {
            text.replacen(
                ". If you do, ",
                ". If a creature is destroyed this way, ",
                1,
            )
        })
        .collect();
    texts.extend(found);
}
