//! A prevention that costs a counter: "If this creature would be dealt damage, prevent that damage and remove a +1/+1
//! counter from it." The renderers write two sentences.

use super::super::super::counters::kind_text;
use crate::ability::Ability;
use crate::refs::Ref;
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// Adds the wordings of the prevention of all damage to the source that removes one counter from it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when:
            ReplaceWhen::Damage {
                to: Some(Ref::This),
                source,
                combat: None,
            },
        by: ReplaceBy::PreventRemovingCounter(counter),
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    let Some(kind) = kind_text(counter).filter(|_| source.0.is_empty()) else {
        return;
    };
    texts.push(format!(
        "If ~ would be dealt damage, prevent that damage and remove a {kind} counter from it."
    ));
}
