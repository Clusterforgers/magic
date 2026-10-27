//! The creature of an Aura returned as it dies: "When enchanted creature dies, return it to the battlefield under your
//! control." No renderer writes a move of the creature of the event to the battlefield.

use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, ZoneKind};
use crate::trigger::TriggerPattern;

/// Adds the wording of a return of the enchanted creature as it dies.
pub(super) fn c5_af_add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
        intervening_if: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    if what.0.as_slice() != [Pred::Is(Ref::Attached)] || !targets.is_empty() {
        return;
    }
    let [
        Effect::Move {
            what: Ref::Triggering,
            to:
                Destination::Battlefield {
                    tapped: false,
                    under,
                    transformed: false,
                    counters,
                },
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let control = match under {
        Some(Ref::You) => "your",
        None => "its owner's",
        Some(_) => return,
    };
    if counters.is_empty() {
        texts.push(format!(
            "When enchanted creature dies, return it to the battlefield under {control} control."
        ));
    }
}
