//! An intervening "if" on devotion: "At the beginning of your upkeep, if your devotion to white and black is seven or
//! greater, create a 4/3 white and black Vampire Demon creature token with flying."

use crate::ability::Ability;
use crate::compile::read::amounts::count_word;
use crate::value::{Amount, Cmp, Condition};

/// Adds the wordings of a trigger at a step with an intervening "if" that the devotion to colors is at least a number.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        intervening_if:
            Some(Condition::Compare {
                left: Amount::Devotion(colors),
                op: Cmp::Ge,
                right: Amount::Fixed(number),
            }),
        ..
    } = ability
    else {
        return;
    };
    let Some(word) = u32::try_from(*number).ok().and_then(count_word) else {
        return;
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let names: Vec<String> = colors
        .iter()
        .map(|color| format!("{color:?}").to_lowercase())
        .collect();
    let said: Vec<String> = super::super::super::variants(&plain)
        .iter()
        .filter(|text| text.starts_with("At the beginning of "))
        .filter_map(|text| {
            let (head, rest) = text.split_once(", ")?;
            Some(format!(
                "{head}, if your devotion to {} is {word} or greater, {rest}",
                names.join(" and ")
            ))
        })
        .collect();
    texts.extend(said);
}
