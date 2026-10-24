//! A token created in the sentence of the step before it: "Whenever another nontoken creature you control leaves the
//! battlefield, put a +1/+1 counter on this creature and create a Treasure token." The renderers write two sentences.

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings that join a token creation to the step before it with "and".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [
        Effect::PutCounters { .. }
        | Effect::GainLife { .. }
        | Effect::Draw { .. }
        | Effect::Scry { .. },
        Effect::CreateToken { .. },
    ] = ability.effects()
    else {
        return;
    };
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            [". You create ", ". Then create "].iter().find_map(|from| {
                text.contains(from)
                    .then(|| text.replacen(from, " and create ", 1))
            })
        })
        .collect();
    texts.extend(found);
}
