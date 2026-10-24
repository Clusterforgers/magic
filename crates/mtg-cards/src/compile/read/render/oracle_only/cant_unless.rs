//! A restriction that lasts until a condition holds: "~ can't attack unless you control four or more artifacts." The data
//! is the restriction under the opposite condition (fewer than four artifacts).

use super::super::super::conditional::condition_texts;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::Ref;
use crate::value::{Cmp, Condition};

/// The condition that holds exactly when `condition` does not.
fn opposite(condition: &Condition) -> Option<Condition> {
    match condition {
        Condition::Not(inner) => Some((**inner).clone()),
        Condition::Compare { left, op, right } => {
            let op = match op {
                Cmp::Lt => Cmp::Ge,
                Cmp::Le => Cmp::Gt,
                Cmp::Gt => Cmp::Le,
                Cmp::Ge => Cmp::Lt,
                Cmp::Eq => Cmp::Ne,
                Cmp::Ne => Cmp::Eq,
            };
            Some(Condition::Compare {
                left: left.clone(),
                op,
                right: right.clone(),
            })
        }
        _ => None,
    }
}

/// Adds the "unless" wordings of a restriction of the card itself under a condition.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected: Ref::This,
        mods,
        condition: Some(condition),
        defining: false,
        ..
    } = ability
    else {
        return;
    };
    let verb = match mods.as_slice() {
        [Modification::CantAttack] => "attack",
        [Modification::CantBlock] => "block",
        [Modification::CantAttack, Modification::CantBlock] => "attack or block",
        _ => return,
    };
    let Some(opposite) = opposite(condition) else {
        return;
    };
    for text in condition_texts(&opposite) {
        texts.push(format!("~ can't {verb} unless {text}."));
    }
}
