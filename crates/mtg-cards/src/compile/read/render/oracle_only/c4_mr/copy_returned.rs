//! A copy of a spell and the return of the spell to its owner's hand: "Copy target instant or sorcery spell, then return
//! it to its owner's hand. You may choose new targets for the copy."

use super::twin_variants;
use crate::ability::Ability;

/// The return of the copied spell, which the plain twin leaves out.
const RETURN: &str = r#",{"Move":{"what":{"Target":0},"to":"Hand"}}"#;

/// Adds the wordings of a spell that copies a spell and returns it to its owner's hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(ability, RETURN, "")
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.strip_prefix("Copy target ")?.split_once(". ")?;
            Some(format!(
                "Copy target {head}, then return it to its owner's hand. {rest}"
            ))
        })
        .collect();
    texts.extend(said);
}
