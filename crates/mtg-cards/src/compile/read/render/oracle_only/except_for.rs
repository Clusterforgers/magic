//! A bounce of every creature but those of some types: "Return all creatures to their owners' hands except for Krakens,
//! Leviathans, Octopuses, and Serpents." The renderers have no wording for the exception.

use super::super::super::subtypes::plural_subtype;
use super::super::variants;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector};
use mtg_core::card::CardType;

/// Adds the wordings of a return to the hand of every creature that has none of some creature types.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let [
        Effect::Move {
            what: Ref::Each(selector),
            to: Destination::Hand,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [Pred::Type(CardType::Creature), Pred::Not(except)] = selector.0.as_slice() else {
        return;
    };
    let Pred::AnyOf(kinds) = &**except else {
        return;
    };
    let names: Option<Vec<String>> = kinds
        .iter()
        .map(|kind| match kind {
            Pred::Subtype(subtype) => Some(plural_subtype(&subtype.0)),
            _ => None,
        })
        .collect();
    let Some(mut names) = names.filter(|names| names.len() > 1) else {
        return;
    };
    let final_name = names.pop().unwrap_or_default();
    let joined = format!("{}, and {final_name}", names.join(", "));
    let plain = Ability::Spell {
        targets: targets.clone(),
        effects: vec![Effect::Move {
            what: Ref::Each(Selector(vec![Pred::Type(CardType::Creature)])),
            to: Destination::Hand,
        }],
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| format!("{text} except for {joined}."))
        .collect();
    texts.extend(said);
}
