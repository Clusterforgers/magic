//! A trigger that only works for a card cast from a zone: "When this creature enters, if you cast it from your hand,
//! destroy all non-Dinosaur creatures." The renderers have no wording for where the card was cast from.

use super::super::variants;
use crate::ability::Ability;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::value::Condition;

/// "your hand" and "your graveyard" for the zones a card is cast from.
fn zone_words(zone: ZoneKind) -> Option<&'static str> {
    match zone {
        ZoneKind::Hand => Some("your hand"),
        ZoneKind::Graveyard => Some("your graveyard"),
        _ => None,
    }
}

/// Adds the wordings of a trigger with an intervening "if" that the card was cast from a zone.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        intervening_if:
            Some(Condition::Matches {
                subject: Ref::This,
                selector,
            }),
        ..
    } = ability
    else {
        return;
    };
    let [Pred::CastFrom(zone)] = selector.0.as_slice() else {
        return;
    };
    let Some(words) = zone_words(*zone) else {
        return;
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.split_once(", "))
        .map(|(event, rest)| format!("{event}, if you cast it from {words}, {rest}"))
        .collect();
    texts.extend(said);
}
