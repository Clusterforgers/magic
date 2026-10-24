//! The permanent that dealt damage returns to its owner's hand: "Whenever a permanent deals damage to you, return it to
//! its owner's hand."

use super::super::pattern_text;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// Adds the wording of a trigger on damage whose one effect returns the source of the damage to its owner's hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: when @ TriggerPattern::DealsDamage { .. },
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let returned = matches!(
        effects.as_slice(),
        [Effect::Move {
            what: Ref::Triggering,
            to: Destination::Hand
        }]
    );
    if !targets.is_empty() || !returned {
        return;
    }
    if let Some(event) = pattern_text(when) {
        texts.push(format!("{event}, return it to its owner's hand."));
    }
}
