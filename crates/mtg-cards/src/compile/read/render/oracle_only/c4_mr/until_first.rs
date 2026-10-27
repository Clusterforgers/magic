//! The duration of a change that comes first after a condition: "Whenever ~ attacks, if you control three or more Dragons,
//! until end of turn, ~ becomes a Dragon with base power and toughness 5/5 and gains flying." The renderers end the
//! sentence with "until end of turn".

use crate::ability::Ability;

/// Adds the wordings that move the duration of a change of the source to the front of its sentence.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        intervening_if: Some(_),
        ..
    } = ability
    else {
        return;
    };
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let body = text.strip_suffix(" until end of turn.")?;
            let (head, rest) = body.rsplit_once(", ~ becomes ")?;
            Some(format!("{head}, until end of turn, ~ becomes {rest}."))
        })
        .collect();
    texts.extend(said);
}
