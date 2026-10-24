//! Targets chosen in a sentence of their own: "Choose any number of target creatures. Double the number of +1/+1
//! counters on each of them." The renderers write "on each of any number of target creatures".

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings of a doubling of counters on any number of targets that choose them first.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !ability
        .effects()
        .iter()
        .any(|effect| matches!(effect, Effect::MultiplyCounters { .. }))
    {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (head, targets) = text.split_once(" each of any number of target ")?;
            Some(format!(
                "Choose any number of target {targets} {head} each of them."
            ))
        })
        .collect();
    texts.extend(said);
}
