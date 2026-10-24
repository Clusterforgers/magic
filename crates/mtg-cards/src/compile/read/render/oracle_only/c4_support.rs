//! Support: "When this creature enters, support 2." puts a +1/+1 counter on each of up to that many other target
//! creatures. The renderers word the counters.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;
use crate::value::{Amount, CounterKind};

/// The words of the counters on the creatures, before the number.
const BEFORE: &str = "When ~ enters, put a +1/+1 counter on each of up to ";

/// The words after the number.
const AFTER: &str = " target other creatures.";

/// Adds the wording that names the counters on up to N other creatures "support N".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::ZoneChange { .. },
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let (
        [spec],
        [
            Effect::PutCounters {
                on: Ref::Target(0),
                counter: CounterKind::PlusOnePlusOne,
                count,
            },
        ],
    ) = (targets.as_slice(), effects.as_slice())
    else {
        return;
    };
    let Some(Amount::Fixed(most)) = spec.bounds.max.as_ref() else {
        return;
    };
    if spec.bounds.min != Amount::Fixed(0) || !count.is_one() {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.starts_with(BEFORE) && text.ends_with(AFTER))
        .map(|_| format!("When ~ enters, support {most}."))
        .collect();
    texts.extend(found);
}
