//! Counters for each life gained: "put a +1/+1 counter on target creature for each 1 life you gained." The renderers
//! word the count "that many".

use crate::ability::Ability;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger on gaining life that puts a +1/+1 or -1/-1 counter for each life.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::LifeGained { who: Ref::You },
            ..
        }
    ) {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| per_life(text)).collect();
    texts.extend(found);
}

/// The text with "put that many +1/+1 counters on target creature." as "put a +1/+1 counter on target creature for each
/// 1 life you gained."
fn per_life(text: &str) -> Option<String> {
    let at = text.find("put that many ")?;
    let (head, rest) = text.split_at(at);
    let (kind, place) = rest
        .strip_prefix("put that many ")?
        .split_once(" counters on ")?;
    let place = place
        .strip_suffix('.')
        .filter(|place| !place.contains(". "))?;
    kind.starts_with(['+', '-'])
        .then(|| format!("{head}put a {kind} counter on {place} for each 1 life you gained."))
}
