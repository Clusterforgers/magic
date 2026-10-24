//! The attack of creatures as one event: "Whenever one or more creatures attack, ...". The renderers word the attack of
//! one creature.

use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

/// Adds the wordings of a batch of attacking creatures, from those of the attack of one creature.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::Batch(inner),
        ..
    } = ability
    else {
        return;
    };
    let TriggerPattern::Attacks { what } = &**inner else {
        return;
    };
    if what.0 != [Pred::Type(CardType::Creature)] {
        return;
    }
    let plain = (**inner).clone();
    let Some(event) = pattern_text(&plain) else {
        return;
    };
    let mut single = ability.clone();
    if let Ability::Triggered { when, .. } = &mut single {
        *when = plain;
    }
    for text in variants(&single) {
        if let Some(rest) = text.strip_prefix(&event) {
            texts.push(format!("Whenever one or more creatures attack{rest}"));
        }
    }
}
