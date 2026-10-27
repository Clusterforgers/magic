//! Counters put by the controller on a creature: "Whenever you put one or more -1/-1 counters on a creature, create a
//! 1/1 green Snake creature token." The renderers write the event without the player and the number: "Whenever a -1/-1
//! counter is put on a creature".

use crate::ability::Ability;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;

/// The opening of the event as the renderers word it, before the kind of counter.
const SAID: &str = "Whenever a ";

/// The words that end the kind of counter.
const COUNTER: &str = "counter is put on ";

/// Adds the wording of a trigger on counters put on creatures other than the card itself.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::CountersPut { what, .. },
        ..
    } = ability
    else {
        return;
    };
    if what.0.contains(&Pred::This) {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let rest = text.strip_prefix(SAID)?;
            let (kind, tail) = rest.split_once(COUNTER)?;
            Some(format!(
                "Whenever you put one or more {kind}counters on {tail}"
            ))
        })
        .collect();
    texts.extend(found);
}
