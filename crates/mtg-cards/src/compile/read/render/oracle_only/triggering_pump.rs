//! A pump of the creature an event is about: "Whenever another creature you control enters, that creature gets +2/+0 and
//! gains haste until end of turn." The renderers open the effect with its duration and call the creature "it".

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger whose only step changes the creature of the event until end of turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::ZoneChange { .. },
        effects,
        ..
    } = ability
    else {
        return;
    };
    if !matches!(
        effects.as_slice(),
        [Effect::Continuous {
            affected: Ref::Triggering,
            ..
        }]
    ) {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| text.split_once(", Until end of turn, it "))
        .filter_map(|(event, step)| {
            let step = step.strip_suffix('.')?;
            Some(format!("{event}, that creature {step} until end of turn."))
        })
        .collect();
    texts.extend(said);
}
