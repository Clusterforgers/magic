//! Counters put on the creature of an event: "Whenever another creature you control enters, put a +1/+1 counter on that
//! creature." The renderers write "on it".

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings with "that creature" for counters put on the object of the trigger.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let on_event = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::PutCounters {
                on: Ref::Triggering,
                ..
            }
        )
    });
    if !on_event {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" on it."))
        .map(|text| text.replacen(" on it.", " on that creature.", 1))
        .collect();
    texts.extend(said);
}
