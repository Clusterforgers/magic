//! An intervening "if" on the size of the creature that died: "Whenever a creature an opponent controls dies, if its
//! toughness was less than 1, draw a card." The renderers have no wording for the size the creature had.

use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition, StatKind};

/// The words that follow the characteristic in the comparison, for a number.
fn comparison(op: Cmp, number: i32) -> Option<String> {
    Some(match op {
        Cmp::Lt => format!("less than {number}"),
        Cmp::Gt => format!("greater than {number}"),
        Cmp::Le => format!("{number} or less"),
        Cmp::Ge => format!("{number} or greater"),
        _ => return None,
    })
}

/// Adds the wordings of a dies trigger whose intervening "if" compares a characteristic the creature had.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                from: Some(_),
                to: Some(_),
                ..
            },
        intervening_if:
            Some(Condition::Matches {
                subject: Ref::Triggering,
                selector: Selector(preds),
            }),
        ..
    } = ability
    else {
        return;
    };
    let [
        Pred::Compare {
            stat,
            op,
            to: Amount::Fixed(number),
        },
    ] = preds.as_slice()
    else {
        return;
    };
    let (Some(stat), Some(words)) = (
        match stat {
            StatKind::Power => Some("power"),
            StatKind::Toughness => Some("toughness"),
            StatKind::ManaValue => Some("mana value"),
            _ => None,
        },
        comparison(*op, *number),
    ) else {
        return;
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let said: Vec<String> = super::super::super::variants(&plain)
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(" dies, ")?;
            Some(format!("{head} dies, if its {stat} was {words}, {rest}"))
        })
        .collect();
    texts.extend(said);
}
