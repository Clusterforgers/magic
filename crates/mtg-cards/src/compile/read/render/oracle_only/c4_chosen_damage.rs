//! Damage that repeats to the chosen player: "Whenever a creature you control deals combat damage to a player, it deals
//! that much damage to the chosen player." The renderers have no wording for the chosen player as a recipient.

use super::super::pattern_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// Adds the wording of a trigger on damage whose one effect is the same damage dealt by the damager to the chosen player.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: when @ TriggerPattern::DealsDamage { .. },
        targets,
        intervening_if: None,
        limit: None,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::DealDamage {
            source: Some(Ref::Dealer),
            to: Ref::Chosen,
            amount: Amount::Triggering,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    if !targets.is_empty() {
        return;
    }
    if let Some(event) = pattern_text(when) {
        texts.push(format!(
            "{event}, it deals that much damage to the chosen player."
        ));
    }
}
