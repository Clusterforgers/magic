//! An entry that only works for a kicked spell: "When ~ enters, if it was kicked, create Karox Bladewing, a legendary
//! 4/4 red Dragon creature token with flying." The renderers have no wording for the kicker in an intervening "if".

use super::super::variants;
use crate::ability::Ability;
use crate::refs::{Pred, Selector};
use crate::trigger::TriggerPattern;
use crate::value::Condition;

/// Adds the wordings of an entry trigger with the intervening "if" that the card was kicked.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::ZoneChange { what, .. },
        intervening_if: Some(Condition::Kicked),
        ..
    } = ability
    else {
        return;
    };
    if *what != Selector(vec![Pred::This]) {
        return;
    }
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.split_once(", "))
        .map(|(event, rest)| format!("{event}, if it was kicked, {rest}"))
        .collect();
    texts.extend(said);
}
