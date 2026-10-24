//! An amount of the spell a spell countered: "Create X Treasure tokens, where X is that spell's mana value." The renderers
//! call it "its".

use crate::ability::Ability;
use crate::refs::{Pred, Targetable};

/// Adds the wordings of a count taken from a target that is a spell.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let spell = ability.targets().iter().any(|spec| {
        matches!(&spec.what, Targetable::Object(selector) if selector.0.contains(&Pred::Spell))
    });
    if !spell {
        return;
    }
    let (from, into) = (", where X is its ", ", where X is that spell's ");
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
