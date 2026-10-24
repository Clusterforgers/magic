//! The first main phase of the turn: "At the beginning of your first main phase" is the precombat main phase, which the
//! renderers name by its step.

use crate::ability::Ability;
use crate::trigger::{Step, TriggerPattern};

/// Adds the wordings of a trigger on the precombat main phase that call it the first main phase.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::StepBegins {
            step: Step::Main1, ..
        },
        ..
    } = ability
    else {
        return;
    };
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("precombat main phase"))
        .map(|text| text.replacen("precombat main phase", "first main phase", 1))
        .collect();
    texts.extend(found);
}
