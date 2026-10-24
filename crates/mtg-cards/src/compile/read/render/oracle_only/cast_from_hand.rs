//! A last step that only a spell cast from the hand takes: "Draw a card, then discard a card. You lose 1 life. If this
//! spell was cast from your hand, create a 0/1 black Wizard creature token ...". The renderers have no wording for
//! where the spell was cast from.

use super::super::variants;
use super::bare::{lowered, with_effects};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::value::Condition;

/// Adds the wordings of a spell whose last step is done only if it was cast from the hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(ability, Ability::Spell { .. }) {
        return;
    }
    let Some((
        Effect::If {
            condition:
                Condition::Matches {
                    subject: Ref::This,
                    selector,
                },
            then,
            otherwise,
        },
        before,
    )) = ability.effects().split_last()
    else {
        return;
    };
    if selector.0 != [Pred::CastFrom(ZoneKind::Hand)] || !otherwise.is_empty() {
        return;
    }
    let steps: Vec<Effect> = before.iter().chain(then).cloned().collect();
    let (Some(whole), Some(last)) = (
        with_effects(ability, steps),
        with_effects(ability, then.clone()),
    ) else {
        return;
    };
    let parts = variants(&last);
    for text in variants(&whole) {
        for part in &parts {
            if let Some(head) = text.strip_suffix(part.as_str()) {
                texts.push(format!(
                    "{head}If this spell was cast from your hand, {}",
                    lowered(part)
                ));
            }
        }
    }
}
