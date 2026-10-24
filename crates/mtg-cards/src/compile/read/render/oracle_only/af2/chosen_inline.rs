//! A group compared with a target that the same sentence names: "Destroy all creatures with power greater than target
//! creature's power." The renderers choose the target in a sentence of its own first.

use super::super::super::{capitalize, spec_text};
use crate::ability::Ability;
use crate::compile::read::selectors::type_name;
use crate::refs::{Pred, Targetable};

/// Adds the wordings that drop the sentence which chooses the one target and name it where the group is compared.
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
    let (Some(target), [kind]) = (spec_text(spec), kinds.as_slice()) else {
        return;
    };
    let (opening, that) = (format!("Choose {target}. "), format!("that {kind}'s "));
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let rest = text.strip_prefix(&opening)?;
            rest.starts_with(char::is_uppercase).then_some(())?;
            rest.contains(&that)
                .then(|| capitalize(&rest.replacen(&that, &format!("{target}'s "), 1)))
        })
        .collect();
    texts.extend(said);
}
