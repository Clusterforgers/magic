//! Battalion: "Whenever ~ and at least two other creatures attack, ~ gets +5/+5 until end of turn." The data is a
//! group attack of three creatures you control with the condition that the source is among the attackers; the
//! renderers write "Whenever you attack with three or more creatures".

use super::super::{count_word, variants};
use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::trigger::TriggerPattern;
use crate::value::Condition;
use mtg_core::card::CardType;

/// Adds the wordings of a group attack that the source takes part in.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::GroupAttacks { what, at_least },
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
    if selector.0 != [Pred::Attacking]
        || what.0 != [Pred::Type(CardType::Creature), Pred::ControlledBy(Ref::You)]
    {
        return;
    }
    let (Some(all), Some(others)) = (
        count_word(*at_least),
        at_least.checked_sub(1).and_then(count_word),
    ) else {
        return;
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let attack = format!("Whenever you attack with {all} or more creatures, ");
    for text in variants(&plain) {
        if let Some(rest) = text.strip_prefix(&attack) {
            texts.push(format!(
                "Whenever ~ and at least {others} other creatures attack, {rest}"
            ));
        }
    }
}
