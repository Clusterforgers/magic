//! An entry and a step in one trigger: "When this creature enters and at the beginning of your upkeep, surveil 1." The
//! renderers word one event or a list of events of one kind.

use super::super::{pattern_text, variants};
use super::bare::lowered;
use crate::ability::Ability;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;

/// Whether the pattern is the source entering the battlefield.
fn enters(pattern: &TriggerPattern) -> bool {
    matches!(
        pattern,
        TriggerPattern::ZoneChange { what, from: None, to: Some(crate::refs::ZoneKind::Battlefield) }
            if what.0 == [Pred::This]
    )
}

/// Adds the wordings of a trigger on the source entering or the beginning of a step.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::AnyOf(events),
        ..
    } = ability
    else {
        return;
    };
    let [first, second @ TriggerPattern::StepBegins { .. }] = events.as_slice() else {
        return;
    };
    if !enters(first) {
        return;
    }
    let (Some(entry), Some(step)) = (pattern_text(first), pattern_text(second)) else {
        return;
    };
    let mut single = ability.clone();
    if let Ability::Triggered { when, .. } = &mut single {
        *when = first.clone();
    }
    let head = format!("{entry}, ");
    for text in variants(&single) {
        if let Some(rest) = text.strip_prefix(&head) {
            texts.push(format!("{entry} and {}, {rest}", lowered(&step)));
        }
    }
}
