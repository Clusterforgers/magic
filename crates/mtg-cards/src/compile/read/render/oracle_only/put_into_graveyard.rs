//! The death of a creature you own: "Whenever a nontoken creature is put into your graveyard from the battlefield, ...".
//! The cards of your graveyard are the cards you own; the renderers word the creatures you control that die.

use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger on a card owned by you going from the battlefield to a graveyard.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
        ..
    } = ability
    else {
        return;
    };
    if !what.0.contains(&Pred::OwnedBy(Ref::You)) || what.0.contains(&Pred::This) {
        return;
    }
    let controlled = Selector(
        what.0
            .iter()
            .map(|pred| match pred {
                Pred::OwnedBy(Ref::You) => Pred::ControlledBy(Ref::You),
                other => other.clone(),
            })
            .collect(),
    );
    let plain = TriggerPattern::ZoneChange {
        what: controlled,
        from: Some(ZoneKind::Battlefield),
        to: Some(ZoneKind::Graveyard),
    };
    let Some(event) = pattern_text(&plain) else {
        return;
    };
    let Some(subject) = event.strip_suffix(" you control dies") else {
        return;
    };
    let moved = format!("{subject} is put into your graveyard from the battlefield");
    let mut bare = ability.clone();
    if let Ability::Triggered { when, .. } = &mut bare {
        *when = plain;
    }
    for text in variants(&bare) {
        if let Some(rest) = text.strip_prefix(&event) {
            texts.push(format!("{moved}{rest}"));
        }
    }
}
