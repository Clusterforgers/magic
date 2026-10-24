//! A draw that the controller of the creature of an event may make: "Whenever a Sliver deals combat damage to a player,
//! its controller may draw a card." The renderers write the draw for "you".

use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;

/// Adds the wordings of a draw that the controller of the damage source may make.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::DealsDamage { .. },
        effects,
        ..
    } = ability
    else {
        return;
    };
    let theirs = Ref::ControllerOf(Box::new(Ref::Triggering));
    let [
        Effect::May {
            who,
            effects: steps,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [Effect::Draw { who: drawer, count }] = steps.as_slice() else {
        return;
    };
    if *who != theirs || *drawer != theirs {
        return;
    }
    let mut yours = ability.clone();
    if let Ability::Triggered { effects, .. } = &mut yours {
        *effects = vec![Effect::May {
            who: Ref::You,
            effects: vec![Effect::Draw {
                who: Ref::You,
                count: count.clone(),
            }],
        }];
    }
    let (from, into) = (", you may draw ", ", its controller may draw ");
    texts.extend(
        variants(&yours)
            .iter()
            .filter(|text| text.contains(from))
            .map(|text| text.replacen(from, into, 1)),
    );
}
