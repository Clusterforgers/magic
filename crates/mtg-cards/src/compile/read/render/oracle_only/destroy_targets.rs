//! Several destroyed targets in one sentence: "Destroy target artifact, target creature, target enchantment, and target
//! land." The renderers word each destruction as a sentence of its own.

use super::super::spec_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wording of a spell that destroys each of its two or more targets once, in the order they are named.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    if targets.len() < 2 || targets.len() != effects.len() {
        return;
    }
    let in_order = effects.iter().enumerate().all(|(at, effect)| {
        matches!(
            effect,
            Effect::Destroy { what: Ref::Target(index), regenerate: true } if usize::from(*index) == at
        )
    });
    if !in_order {
        return;
    }
    let Some(named) = targets.iter().map(spec_text).collect::<Option<Vec<_>>>() else {
        return;
    };
    let Some((last, rest)) = named.split_last() else {
        return;
    };
    let list = match rest {
        [one] => format!("{one} and {last}"),
        _ => format!("{}, and {last}", rest.join(", ")),
    };
    texts.push(format!("Destroy {list}."));
}
