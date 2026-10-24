//! An attachment the controller may move: "Whenever an Equipment you control enters, you may attach it to target
//! creature you control." The renderers have no wording for an attach beside the event.

use super::super::{spec_text, trigger_head};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings of a trigger whose only step lets the controller attach the object of the event to a target.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::May {
            who: Ref::You,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [
        Effect::Attach {
            what: Ref::Triggering,
            to: Ref::Target(0),
        },
    ] = inner.as_slice()
    else {
        return;
    };
    let [spec] = targets.as_slice() else {
        return;
    };
    let (Some(head), Some(target)) = (trigger_head(when, intervening_if.as_ref()), spec_text(spec))
    else {
        return;
    };
    texts.push(format!("{} you may attach it to {target}.", head.join(" ")));
}
