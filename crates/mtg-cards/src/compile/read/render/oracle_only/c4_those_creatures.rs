//! Counters on a group and a grant to the same group in one sentence: "put a +1/+1 counter on each creature you
//! control and those creatures gain deathtouch until end of turn." The renderers write the grant as a sentence of its
//! own.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// The openings of the sentence that grants to the group, as the renderers write them.
const GRANTS: [&str; 2] = [
    ". Creatures you control gain ",
    ". Each creature you control gains ",
];

/// Adds the wordings of counters on a group and a grant to the same group, joined by "and those creatures".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [
        Effect::PutCounters {
            on: Ref::Each(group),
            ..
        },
        Effect::Continuous {
            affected: Ref::Each(same),
            ..
        },
    ] = ability.effects()
    else {
        return;
    };
    if group != same {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            GRANTS
                .iter()
                .filter(|grant| text.contains(*grant))
                .map(|grant| text.replacen(grant, " and those creatures gain ", 1))
        })
        .collect();
    texts.extend(said);
}
