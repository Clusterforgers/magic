//! Damage that a creature just put onto the battlefield deals: "Put target creature card from a graveyard onto the
//! battlefield under your control. That creature deals damage equal to its power to each other creature."

use super::super::{Form, phrase_text, variants};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, StatKind};

/// Adds the wording of a move of the target followed by damage the target deals to a group other than itself.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { effects, .. } = ability else {
        return;
    };
    let [
        first @ Effect::Move {
            what: Ref::Target(0),
            ..
        },
        Effect::DealDamage {
            source: Some(Ref::Target(0)),
            to: Ref::Each(group),
            amount:
                Amount::Stat {
                    of: Ref::Target(0),
                    stat: StatKind::Power,
                },
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let not_it = Pred::Not(Box::new(Pred::Is(Ref::Target(0))));
    let mut others: Vec<Pred> = group
        .0
        .iter()
        .filter(|pred| **pred != not_it)
        .cloned()
        .collect();
    if others.len() + 1 != group.0.len() {
        return;
    }
    others.push(Pred::Other);
    let Some(phrase) = phrase_text(&Selector(others), Form::Each) else {
        return;
    };
    let mut alone = ability.clone();
    if let Ability::Spell { effects, .. } = &mut alone {
        *effects = vec![first.clone()];
    }
    texts.extend(variants(&alone).iter().map(|text| {
        format!("{text} That creature deals damage equal to its power to each {phrase}.")
    }));
}
