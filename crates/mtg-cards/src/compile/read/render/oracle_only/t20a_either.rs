//! A choice between two steps on a characteristic of the object of the event against a number: "draw a card if its
//! power is 3 or greater. Otherwise, put two +1/+1 counters on it." The renderers word the choice on how that object
//! compares with the card ("if its power is less than ~'s power"); the twin has that condition and its words are
//! swapped for the number's.

use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition, StatKind};

/// Adds the wordings of a trigger whose only step is a choice on a characteristic of the object of its event.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let [Effect::If { condition, .. }] = effects.as_slice() else {
        return;
    };
    let Some((stat, op, number)) = against_number(condition) else {
        return;
    };
    let Some(name) = name(stat) else {
        return;
    };
    let Some(relation) = relation(op, number) else {
        return;
    };
    let mut twin = ability.clone();
    if let Ability::Triggered { effects, .. } = &mut twin
        && let [Effect::If { condition, .. }] = effects.as_mut_slice()
    {
        *condition = Condition::Compare {
            left: Amount::Stat {
                of: Ref::Triggering,
                stat,
            },
            op: Cmp::Lt,
            right: Amount::Stat {
                of: Ref::This,
                stat,
            },
        };
    }
    let known = format!("its {name} is less than ~'s {name}");
    let said = format!("its {name} is {relation}");
    texts.extend(
        variants(&twin)
            .into_iter()
            .filter(|text| text.contains(&known))
            .map(|text| text.replace(&known, &said)),
    );
}

/// The characteristic of the object of the event, how it compares and the number it is compared with.
fn against_number(condition: &Condition) -> Option<(StatKind, Cmp, i64)> {
    match condition {
        Condition::Compare {
            left:
                Amount::Stat {
                    of: Ref::Triggering,
                    stat,
                },
            op,
            right: Amount::Fixed(number),
        } => Some((*stat, *op, i64::from(*number))),
        Condition::Matches {
            subject: Ref::Triggering,
            selector: Selector(preds),
        } => match preds.as_slice() {
            [
                Pred::Compare {
                    stat,
                    op,
                    to: Amount::Fixed(number),
                },
            ] => Some((*stat, *op, i64::from(*number))),
            _ => None,
        },
        _ => None,
    }
}

/// The characteristic as a card says it.
fn name(stat: StatKind) -> Option<&'static str> {
    match stat {
        StatKind::Power => Some("power"),
        StatKind::Toughness => Some("toughness"),
        StatKind::ManaValue => Some("mana value"),
        _ => None,
    }
}

/// "3 or greater", "2 or less", "exactly 3": how a number is compared with.
fn relation(op: Cmp, number: i64) -> Option<String> {
    Some(match op {
        Cmp::Ge => format!("{number} or greater"),
        Cmp::Le => format!("{number} or less"),
        Cmp::Eq => format!("{number}"),
        Cmp::Gt => format!("greater than {number}"),
        Cmp::Lt => format!("less than {number}"),
        Cmp::Ne => return None,
    })
}
