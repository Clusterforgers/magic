//! An exile from the battlefield as an event: "Whenever a creature is exiled from the battlefield, put a +1/+1 counter
//! on this creature." The renderers word the move from the battlefield to a graveyard.

use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::refs::{Pred, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger on an object other than the source going from the battlefield to exile.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Exile),
            },
        ..
    } = ability
    else {
        return;
    };
    if what.0.contains(&Pred::This) {
        return;
    }
    let dies = TriggerPattern::ZoneChange {
        what: what.clone(),
        from: Some(ZoneKind::Battlefield),
        to: Some(ZoneKind::Graveyard),
    };
    let Some(event) = pattern_text(&dies) else {
        return;
    };
    let Some(subject) = event.strip_suffix(" dies") else {
        return;
    };
    let mut bare = ability.clone();
    if let Ability::Triggered { when, .. } = &mut bare {
        *when = dies;
    }
    for text in variants(&bare) {
        if let Some(rest) = text.strip_prefix(&event) {
            texts.push(format!("{subject} is exiled from the battlefield{rest}"));
        }
    }
}
