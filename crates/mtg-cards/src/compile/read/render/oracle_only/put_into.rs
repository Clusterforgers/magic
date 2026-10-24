//! A card that enters or dies says "enters or is put into a graveyard from the battlefield" when it is not a creature:
//! "When this artifact enters or is put into a graveyard from the battlefield, draw a card."

use crate::ability::Ability;
use crate::refs::{Pred, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings that put the card into a graveyard from the battlefield instead of dying.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(events),
        ..
    } = ability
    else {
        return;
    };
    if !events.iter().any(dies_itself) {
        return;
    }
    let (dies, put) = (
        " or dies, ",
        " or is put into a graveyard from the battlefield, ",
    );
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(dies))
        .map(|text| text.replacen(dies, put, 1))
        .collect();
    texts.extend(found);
}

/// Whether the event is the card itself going from the battlefield to a graveyard.
fn dies_itself(event: &TriggerPattern) -> bool {
    matches!(
        event,
        TriggerPattern::ZoneChange {
            what,
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        } if what.0 == [Pred::This]
    )
}
