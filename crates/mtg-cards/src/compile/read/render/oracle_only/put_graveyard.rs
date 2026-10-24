//! A card that enters or is put into a graveyard: "When this artifact enters or is put into a graveyard from the
//! battlefield, create a 1/1 colorless Servo artifact creature token." The renderers write "dies" and a subject for the
//! controller.

use crate::ability::Ability;
use crate::refs::{Pred, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings of a trigger on the source entering or leaving the battlefield for a graveyard.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(events),
        ..
    } = ability
    else {
        return;
    };
    let leaves = events.iter().any(|event| {
        matches!(
            event,
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            } if what.0 == [Pred::This]
        )
    });
    if !leaves {
        return;
    }
    let (from, into) = (
        " enters or dies, you create ",
        " enters or is put into a graveyard from the battlefield, create ",
    );
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
