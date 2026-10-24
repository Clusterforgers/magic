//! Counters on the creature of an event, which a card calls "that creature": "Whenever a creature you control deals
//! combat damage to a player, put a +1/+1 counter on that creature." The renderers write "it".

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings that name the creature the counters go on.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    if !effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::PutCounters {
                on: Ref::Triggering,
                ..
            }
        )
    }) {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            [" counter on it", " counters on it"]
                .iter()
                .find_map(|tail| {
                    text.contains(tail).then(|| {
                        text.replacen(
                            tail,
                            &format!("{} that creature", &tail[..tail.len() - 3]),
                            1,
                        )
                    })
                })
        })
        .collect();
    texts.extend(found);
}
