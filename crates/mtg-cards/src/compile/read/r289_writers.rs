//! Wordings of the r28-9 card pack written from a twin: haunt's second event beside the card entering ("When this
//! creature enters or the creature it haunts dies", CR 702.55c) and the controlled player's "their next turn".

use crate::ability::Ability;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wordings of the r28-9 twins.
pub(super) fn r289_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let mut found = Vec::new();
    if let Some(twin) = r289_entering_twin(ability) {
        for text in written(&twin) {
            if let Some(rest) = text.strip_prefix("When ~ enters, ") {
                found.push(format!(
                    "When ~ enters or the creature it haunts dies, {rest}"
                ));
            }
        }
    }
    for text in texts.iter() {
        if text.contains("during that player's next turn") {
            found.push(text.replace("during that player's next turn", "during their next turn"));
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The ability with "When ~ enters" alone, for a triggered ability whose event is "~ enters or the creature it haunts
/// dies".
fn r289_entering_twin(ability: &Ability) -> Option<Ability> {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(parts),
        ..
    } = ability
    else {
        return None;
    };
    let [enters, haunted] = parts.as_slice() else {
        return None;
    };
    let entering = matches!(enters, TriggerPattern::ZoneChange { what, from: None, to: Some(ZoneKind::Battlefield) } if what.0 == [Pred::This]);
    let dying = matches!(haunted, TriggerPattern::ZoneChange { what, from: Some(ZoneKind::Battlefield), to: Some(ZoneKind::Graveyard) } if what.0 == [Pred::HauntedBy(Ref::This)]);
    if !entering || !dying {
        return None;
    }
    let mut twin = ability.clone();
    if let Ability::Triggered { when, .. } = &mut twin {
        *when = enters.clone();
    }
    Some(twin)
}
