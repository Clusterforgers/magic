//! A sacrifice the opponents choose, with a second step they all take: "Each opponent sacrifices a creature or planeswalker
//! of their choice and gets a poison counter." The renderers write two sentences.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings of a sacrifice by each opponent followed by counters on each opponent.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [
        Effect::Sacrifice {
            who: Ref::Opponents,
            ..
        },
        Effect::PutCounters {
            on: Ref::Opponents, ..
        },
    ] = ability.effects()
    else {
        return;
    };
    let (from, into) = (". Then each opponent gets ", " of their choice and gets ");
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.starts_with("Each opponent sacrifices ") && text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
