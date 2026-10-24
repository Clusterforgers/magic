//! A step the player skips when a fact about the turn holds: "Draw two cards. Then discard a card unless you attacked
//! this turn." The data is an `If` on the opposite fact; the step is worded alone and "unless" follows it.

use super::super::variants;
use super::bare::{lowered, with_effects};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, TurnTally};

/// The fact the step is skipped for, when the condition is that it did not happen.
fn unless_clause(condition: &Condition) -> Option<&'static str> {
    match condition {
        Condition::Compare {
            left:
                Amount::Tallied {
                    who: Ref::You,
                    what: TurnTally::Attackers,
                },
            op: Cmp::Lt,
            right: Amount::Fixed(1),
        } => Some("you attacked this turn"),
        _ => None,
    }
}

/// Adds the wordings of an ability whose last step is an `If` on a missed fact, with the step in the sentence of its
/// own, also opened with "Then" after another sentence.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((
        Effect::If {
            condition,
            then,
            otherwise,
        },
        before,
    )) = ability.effects().split_last()
    else {
        return;
    };
    let Some(clause) = unless_clause(condition) else {
        return;
    };
    if !otherwise.is_empty() || then.len() != 1 {
        return;
    }
    let mut all = before.to_vec();
    all.extend(then.iter().cloned());
    let Some(bare) = with_effects(ability, all) else {
        return;
    };
    for text in variants(&bare) {
        let Some(body) = text.strip_suffix('.') else {
            continue;
        };
        match body.rsplit_once(". ") {
            Some((head, last)) => {
                texts.push(format!("{head}. {last} unless {clause}."));
                texts.push(format!("{head}. Then {} unless {clause}.", lowered(last)));
            }
            None => texts.push(format!("{body} unless {clause}.")),
        }
    }
}
