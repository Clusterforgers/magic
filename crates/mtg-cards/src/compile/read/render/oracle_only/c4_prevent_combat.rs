//! A prevention of all combat damage to a group: "Prevent all combat damage that would be dealt to attacking artifact
//! creatures you control." The renderers write the replacement with "If combat damage would be dealt to ..., prevent
//! that damage."

use super::super::{Form, phrase_text};
use crate::ability::Ability;
use crate::refs::Ref;
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// Adds the wording of a replacement that prevents every combat damage to a group of permanents.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when:
            ReplaceWhen::Damage {
                to: Some(Ref::Each(group)),
                source,
                combat: Some(true),
            },
        by: ReplaceBy::Prevent,
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    if !source.is_empty() {
        return;
    }
    if let Some(group) = phrase_text(group, Form::All) {
        texts.push(format!(
            "Prevent all combat damage that would be dealt to {group}."
        ));
    }
}
