//! The trigger of the card cycling itself: "When you cycle this card, you may have it deal X damage to target creature,
//! where X is the number of Goblins on the battlefield." and "..., you may have target player lose life equal to ...".
//! The renderers write "you may target player loses life" and "deal damage ... equal to the number of".

use crate::ability::Ability;
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger of a card that was cycled with "have" and "where X is".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::Cycled { .. },
        ..
    } = ability
    else {
        return;
    };
    let said: Vec<String> = texts.iter().flat_map(|text| wordings(text)).collect();
    texts.extend(said);
}

/// The text with "have" before the player who loses life, and the damage counted by an X named at the end.
fn wordings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Some((head, tail)) = text.split_once(", you may target ")
        && let Some((who, rest)) = tail.split_once(" loses life ")
    {
        found.push(format!(
            "{head}, you may have target {who} lose life {rest}"
        ));
    }
    if let Some((head, tail)) = text.split_once(", you may have ~ deal damage to ")
        && let Some((target, group)) = tail.split_once(" equal to the number of ")
    {
        found.push(format!(
            "{head}, you may have it deal X damage to {target}, where X is the number of {group}"
        ));
    }
    found
}
