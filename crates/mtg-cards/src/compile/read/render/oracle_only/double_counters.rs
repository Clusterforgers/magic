//! Every kind of counter doubled on chosen permanents: "double the number of each kind of counter on any number of target
//! permanents."

use super::super::pattern_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, TargetSpec, Targetable};
use crate::value::Amount;

/// Adds the wording of a trigger that doubles the counters of any number of target permanents.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        TargetSpec {
            what: Targetable::Object(group),
            bounds,
            divide: None,
        },
    ] = targets.as_slice()
    else {
        return;
    };
    let doubled = matches!(
        effects.as_slice(),
        [Effect::MultiplyCounters {
            on: Ref::Target(0),
            counter: None,
            factor: 2
        }]
    );
    let any_number = bounds.min == Amount::Fixed(0) && bounds.max.is_none();
    if !group.is_empty() || !any_number || !doubled {
        return;
    }
    if let Some(event) = pattern_text(when) {
        texts.push(format!(
            "{event}, double the number of each kind of counter on any number of target permanents."
        ));
    }
}
