//! A cost change for the spells that target something: "Spells your opponents cast that target this creature cost {2}
//! more to cast." The renderers have no wording for the target of a spell in a cost modifier.

use super::super::article;
use crate::ability::{Ability, CostChange};
use crate::refs::{Pred, Ref};

/// Adds the wordings of a cost change for the spells of the controller or the opponents that target the source or a
/// permanent of a type.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::CostModifier {
        applies_to,
        change,
        mana,
        per,
        condition: None,
    } = ability
    else {
        return;
    };
    let [
        Pred::Spell,
        Pred::ControlledBy(who),
        Pred::Targeting(target),
    ] = applies_to.0.as_slice()
    else {
        return;
    };
    let caster = match who {
        Ref::You => "you cast",
        Ref::Opponents => "your opponents cast",
        _ => return,
    };
    let aim = match target.0.as_slice() {
        [Pred::This] => "~".to_owned(),
        [Pred::Type(kind)] => {
            let name = format!("{kind:?}").to_lowercase();
            format!("{} {name}", article(&name))
        }
        _ => return,
    };
    if !per.is_one() {
        return;
    }
    let more = match change {
        CostChange::Increase => "more",
        CostChange::Reduce => "less",
    };
    texts.push(format!(
        "Spells {caster} that target {aim} cost {mana} {more} to cast."
    ));
}
