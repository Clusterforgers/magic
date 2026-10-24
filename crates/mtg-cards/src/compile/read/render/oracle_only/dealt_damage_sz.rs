//! Damage dealt to you by any source: "Whenever you're dealt damage, put that many charge counters on this artifact."
//! The renderers write the source as "a permanent".

use crate::ability::Ability;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger on any damage dealt to you.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::DealsDamage {
                source,
                to: Some(Ref::You),
                combat: None,
            },
        ..
    } = ability
    else {
        return;
    };
    if !source.0.is_empty() {
        return;
    }
    let (from, into) = (
        "Whenever a permanent deals damage to you,",
        "Whenever you're dealt damage,",
    );
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.starts_with(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
