//! A bigger blow when a count of the graveyard is big enough: "~ deals 2 damage to target creature or planeswalker.
//! Delirium — ~ deals 6 damage instead if there are four or more card types among cards in your graveyard." The data is
//! an `If` over the two amounts of one blow at one recipient; the two sentences are worded from the blow alone.

use super::super::variants;
use super::bare::with_effects;
use super::conditions::phrases;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::value::{Amount, Condition};

/// The blow of a branch: its recipient and amount, with no source of its own.
fn blow(effects: &[Effect]) -> Option<(&crate::refs::Ref, i32)> {
    match effects {
        [
            Effect::DealDamage {
                source: None,
                to,
                amount: Amount::Fixed(amount),
            },
        ] => Some((to, *amount)),
        _ => None,
    }
}

/// Adds the wordings of a spell whose damage is bigger under a condition, as two sentences.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { .. } = ability else {
        return;
    };
    let [
        Effect::If {
            condition,
            then,
            otherwise,
        },
    ] = ability.effects()
    else {
        return;
    };
    let (Some((to, big)), Some((same, small))) = (blow(then), blow(otherwise)) else {
        return;
    };
    if to != same || big <= small {
        return;
    }
    let (Some(base), Some(high)) = (
        with_effects(ability, otherwise.clone()),
        with_effects(ability, then.clone()),
    ) else {
        return;
    };
    let phrases = phrases(condition);
    let delirium = matches!(
        condition,
        Condition::Compare {
            left: Amount::CardTypes(_),
            ..
        }
    );
    for first in variants(&base) {
        for second in variants(&high) {
            let Some(at) = second.find(" damage") else {
                continue;
            };
            let head = &second[..at + " damage".len()];
            for phrase in &phrases {
                texts.push(format!("{first} {head} instead if {phrase}."));
                if delirium {
                    texts.push(format!("{first} Delirium — {head} instead if {phrase}."));
                }
            }
        }
    }
}
