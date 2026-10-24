//! A second gift for the same creature when a condition holds: "Target creature gains double strike until end of turn.
//! Ferocious — That creature also gains trample until end of turn if you control a creature with power 4 or greater."
//! The renderers open the second sentence with "Then it".

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings that say "That creature also" for the second step of a spell, with and without the ability word.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { .. } = ability else {
        return;
    };
    let [Effect::Continuous { .. }, Effect::If { .. }] = ability.effects() else {
        return;
    };
    let (from, into) = (". Then it gains ", "That creature also gains ");
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .flat_map(|text| {
            [
                text.replacen(from, &format!(". {into}"), 1),
                text.replacen(from, &format!(". Ferocious \u{2014} {into}"), 1),
            ]
        })
        .collect();
    texts.extend(found);
}
