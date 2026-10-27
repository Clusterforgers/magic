//! A token copy of the permanent that entered: "Whenever an enchantment you control enters, you may create a token that's
//! a copy of it. Do this only once each turn." The renderers know a copy of the sacrificed creature.

use super::twin_variants;
use crate::ability::Ability;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;

/// The copy of the permanent of the event, and the copy of a sacrificed creature the renderers write.
const EVENT: &str = r#""CopyOf":{"of":"Triggering"}"#;
const SACRIFICED: &str = r#""CopyOf":{"of":"Paid"}"#;

/// The card type that the permanents of a trigger have, in the lower case of the noun the card uses.
fn noun(ability: &Ability) -> Option<String> {
    let Ability::Triggered {
        when: TriggerPattern::ZoneChange { what, .. },
        ..
    } = ability
    else {
        return None;
    };
    what.0.iter().find_map(|pred| match pred {
        Pred::Type(kind) => Some(format!("{kind:?}").to_lowercase()),
        _ => None,
    })
}

/// Adds the wordings of a trigger that makes a token copy of the permanent that entered: "it" or "that enchantment".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let kind = noun(ability);
    let mut said = Vec::new();
    for text in twin_variants(ability, EVENT, SACRIFICED) {
        let text = text
            .replace(
                ". Then this ability triggers only once each turn.",
                ". Do this only once each turn.",
            )
            .replace(
                " This ability triggers only once each turn.",
                " Do this only once each turn.",
            );
        said.push(text.replace("the sacrificed creature", "it"));
        if let Some(kind) = &kind {
            said.push(text.replace("the sacrificed creature", &format!("that {kind}")));
        }
    }
    texts.extend(said);
}
