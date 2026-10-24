//! The controller of a creature that died is "that player": "Whenever a creature an opponent controls dies, that player
//! loses 2 life." The renderers word the controller of an object "that permanent's controller".

use crate::ability::Ability;
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;

/// Adds the wording that calls the controller of the object that went to a graveyard "that player".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
                ..
            },
        ..
    } = ability
    else {
        return;
    };
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(", that permanent's controller "))
        .map(|text| text.replacen(", that permanent's controller ", ", that player ", 1))
        .collect();
    texts.extend(found);
}
