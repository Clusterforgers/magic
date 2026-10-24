//! Spells of a kind that can't be countered: "Creature spells you control can't be countered."

use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref};

/// Adds the wording of a rule that protects the spells of one card type the controller casts.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::CantBeCountered(Ref::Each(selector)),
        ..
    } = ability
    else {
        return;
    };
    let [Pred::Type(kind), Pred::Spell, Pred::ControlledBy(Ref::You)] = selector.0.as_slice()
    else {
        return;
    };
    texts.push(format!("{kind:?} spells you control can't be countered."));
}
