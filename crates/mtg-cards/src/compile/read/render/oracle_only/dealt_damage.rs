//! A player dealt damage: "Whenever you're dealt damage, you may create that many 1/1 white Soldier creature tokens."
//! The renderers word the source as "a permanent".

use crate::ability::Ability;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// The openings of a trigger on damage to you as the renderers word them and as the oracle text does.
const OPENINGS: [(&str, &str); 2] = [
    (
        "Whenever a permanent deals damage to you, ",
        "Whenever you're dealt damage, ",
    ),
    (
        "Whenever a permanent deals combat damage to you, ",
        "Whenever you're dealt combat damage, ",
    ),
];

/// Adds the wordings of a trigger on damage to you from any source.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::DealsDamage {
                source,
                to: Some(Ref::You),
                ..
            },
        ..
    } = ability
    else {
        return;
    };
    if !source.is_empty() {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            OPENINGS
                .iter()
                .filter_map(|(said, wanted)| Some(format!("{wanted}{}", text.strip_prefix(said)?)))
        })
        .collect();
    texts.extend(found);
}
