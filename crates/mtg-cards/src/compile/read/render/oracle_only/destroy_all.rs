//! A mass destruction of one creature type: "destroy all Humans". The renderers write "destroy all Human creatures", and
//! a step that follows another may open with ", then" instead of a sentence of its own.

use super::super::super::subtypes::plural_subtype;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref};
use mtg_core::card::CardType;

/// Adds the wordings of a destruction of every creature of one type.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for effect in ability.effects() {
        let Effect::Destroy {
            what: Ref::Each(selector),
            ..
        } = effect
        else {
            continue;
        };
        let [Pred::Type(CardType::Creature), Pred::Subtype(subtype)] = selector.0.as_slice() else {
            continue;
        };
        let (long, short) = (
            format!("destroy all {} creatures", subtype.0),
            format!("destroy all {}", plural_subtype(&subtype.0)),
        );
        let said: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(&long))
            .flat_map(|text| {
                let plain = text.replacen(&long, &short, 1);
                let joined = plain.replacen(". Then destroy", ", then destroy", 1);
                [plain, joined]
            })
            .collect();
        texts.extend(said);
    }
}
