//! A bigger step when X is large: "Create X 2/2 white Soldier creature tokens with vigilance. If X is 10 or more, also
//! create X 4/4 white Angel creature tokens with flying and vigilance." The steps are worded apart and the second one
//! follows "also".

use super::super::super::facts;
use super::super::variants;
use super::bare::{lowered, with_effects};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::value::{Amount, Cmp, Condition};

/// Adds the wordings of a spell whose last step is an `If` on a least value of X, after the steps before it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { effects, .. } = ability else {
        return;
    };
    let Some((
        Effect::If {
            condition:
                Condition::Compare {
                    left: Amount::X,
                    op: Cmp::Ge,
                    right: Amount::Fixed(least),
                },
            then,
            otherwise,
        },
        before,
    )) = effects.split_last()
    else {
        return;
    };
    let Ok(least) = u32::try_from(*least) else {
        return;
    };
    if !otherwise.is_empty() || before.is_empty() {
        return;
    }
    let (Some(head), Some(extra)) = (
        with_effects(ability, before.to_vec()),
        with_effects(ability, then.clone()),
    ) else {
        return;
    };
    for head in variants(&head) {
        for extra in variants(&extra) {
            for number in facts::quantity_texts(least) {
                texts.push(format!(
                    "{head} If X is {number} or more, also {}",
                    lowered(&extra)
                ));
            }
        }
    }
}
