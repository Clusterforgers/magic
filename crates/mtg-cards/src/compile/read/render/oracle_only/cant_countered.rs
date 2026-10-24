//! Spells of a kind that can't be countered: "Creature spells you control can't be countered." The data is a rule over
//! the spells you control of one card type.

use super::super::super::selectors::type_name;
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref};

/// Adds the wording of a rule that keeps the spells of one kind that the controller controls from being countered.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::CantBeCountered(Ref::Each(group)),
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    let kind = match group.0.as_slice() {
        [Pred::Spell, Pred::ControlledBy(Ref::You)] => String::new(),
        [
            Pred::Spell,
            Pred::Type(card_type),
            Pred::ControlledBy(Ref::You),
        ] => {
            format!("{} ", type_name(*card_type))
        }
        _ => return,
    };
    let text = format!("{kind}spells you control can't be countered.");
    let mut letters = text.chars();
    if let Some(first) = letters.next() {
        texts.push(first.to_uppercase().chain(letters).collect());
    }
}
