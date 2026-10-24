//! The controller of the object of an event named by the object: "Whenever a creature dies, that creature's controller
//! may draw a card." The renderers write "its controller".

use crate::ability::Ability;
use crate::compile::read::selectors::type_name;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;

const ITS: &str = ", its controller may ";

/// Adds the wordings that name the kind of object whose controller chooses.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::ZoneChange { what, .. },
        ..
    } = ability
    else {
        return;
    };
    let kinds: Vec<&str> = what
        .0
        .iter()
        .filter_map(|pred| match pred {
            Pred::Type(card_type) => Some(type_name(*card_type)),
            _ => None,
        })
        .collect();
    let [kind] = kinds.as_slice() else {
        return;
    };
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(ITS))
        .map(|text| text.replacen(ITS, &format!(", that {kind}'s controller may "), 1))
        .collect();
    texts.extend(said);
}
