//! A trigger that only works for a creature of one size: "Whenever a creature you control enters, if that creature is
//! 1/1, put two +1/+1 counters on it." The renderers have no wording for the size of the creature of an event.

use super::super::variants;
use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::value::{Amount, Cmp, Condition, StatKind};

/// Adds the wordings of a trigger with an intervening "if" that the creature of the event has one size.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        intervening_if:
            Some(Condition::Matches {
                subject: Ref::Triggering,
                selector,
            }),
        ..
    } = ability
    else {
        return;
    };
    let [
        Pred::Compare {
            stat: StatKind::Power,
            op: Cmp::Eq,
            to: Amount::Fixed(power),
        },
        Pred::Compare {
            stat: StatKind::Toughness,
            op: Cmp::Eq,
            to: Amount::Fixed(toughness),
        },
    ] = selector.0.as_slice()
    else {
        return;
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.split_once(", "))
        .map(|(event, rest)| format!("{event}, if that creature is {power}/{toughness}, {rest}"))
        .collect();
    texts.extend(said);
}
