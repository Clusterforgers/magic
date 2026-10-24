//! A card put into your graveyard from anywhere: "Whenever a land card is put into your graveyard from anywhere, you
//! gain 1 life." The cards of your graveyard are the cards you own; the renderers word the permanent that goes to a
//! graveyard.

use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger on a card of a kind that you own going to a graveyard from any zone.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: None,
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
    let anyone = Selector(
        what.0
            .iter()
            .filter(|pred| **pred != Pred::OwnedBy(Ref::You))
            .cloned()
            .collect(),
    );
    let plain = TriggerPattern::ZoneChange {
        what: anyone,
        from: None,
        to: Some(ZoneKind::Graveyard),
    };
    let Some(event) = pattern_text(&plain) else {
        return;
    };
    let Some(subject) = event.strip_suffix(" is put into a graveyard from anywhere") else {
        return;
    };
    let moved = format!("{subject} card is put into your graveyard from anywhere");
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
