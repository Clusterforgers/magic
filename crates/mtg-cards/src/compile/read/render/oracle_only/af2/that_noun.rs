//! A characteristic of the destroyed target named by its kind: "..., where X is that creature's mana value." The renderers
//! write "where X is its mana value".

use crate::ability::Ability;
use crate::compile::read::selectors::type_name;
use crate::refs::{Pred, Targetable};

const ITS: &str = ", where X is its ";

/// Adds the wordings that name the kind of the one target in a where clause.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [spec] = ability.targets() else {
        return;
    };
    let Targetable::Object(selector) = &spec.what else {
        return;
    };
    let kinds: Vec<&str> = selector
        .0
        .iter()
        .filter_map(|pred| match pred {
            Pred::Type(card_type) => Some(type_name(*card_type)),
            _ => None,
        })
        .collect();
    let [kind] = kinds.as_slice() else {
        return;
    };
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(ITS))
        .map(|text| text.replacen(ITS, &format!(", where X is that {kind}'s "), 1))
        .collect();
    texts.extend(said);
}
