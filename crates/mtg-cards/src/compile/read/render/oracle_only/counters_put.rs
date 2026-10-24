//! Counters put on the card by its controller: "Whenever you put one or more +1/+1 counters on this creature, draw a
//! card." The renderers write the event without the player and the number: "Whenever a +1/+1 counter is put on ~".

use crate::ability::Ability;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;

/// The opening of the event as the renderers word it, before the kind of counter.
const SAID: &str = "Whenever a ";

/// The words after the kind of counter, as the renderers word them.
const AFTER: &str = " counter is put on ~, ";

/// Adds the wording of a trigger on counters put on the card itself.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::CountersPut { what, .. },
        ..
    } = ability
    else {
        return;
    };
    if what.0 != [Pred::This] {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let rest = text.strip_prefix(SAID)?;
            let (kind, tail) = rest.split_once(AFTER)?;
            Some(format!(
                "Whenever you put one or more {kind} counters on ~, {tail}"
            ))
        })
        .collect();
    texts.extend(found);
}
