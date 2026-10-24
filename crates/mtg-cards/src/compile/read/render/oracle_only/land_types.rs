//! A basic land type given with the type land: "Nontoken creatures you control are Forest lands in addition to their other
//! types." The renderers write the type and the subtype as two parts.

use super::super::super::subtypes::plural_subtype;
use crate::ability::Ability;
use crate::modify::Modification;
use mtg_core::card::CardType;

/// The basic land types.
const LAND_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// Adds the wordings of a static ability that makes a group lands of one basic land type.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static { mods, .. } = ability else {
        return;
    };
    let name = match mods.as_slice() {
        [
            Modification::AddType(CardType::Land),
            Modification::AddSubtype(subtype),
        ]
        | [
            Modification::AddSubtype(subtype),
            Modification::AddType(CardType::Land),
        ] => subtype.0.as_str(),
        _ => return,
    };
    if !LAND_TYPES.contains(&name) {
        return;
    }
    let plural = plural_subtype(name);
    let article = super::super::super::triggers::article(name);
    let mut found = Vec::new();
    let (them, it) = (
        format!("are {name} lands in addition to their other types"),
        format!("is {article} {name} land in addition to its other types"),
    );
    for kind in ["types", "creature types"] {
        let said = [
            (
                format!(
                    "are lands in addition to their other types and are {plural} in addition to their other {kind}"
                ),
                them.clone(),
            ),
            (
                format!(
                    "are {plural} in addition to their other {kind} and are lands in addition to their other types"
                ),
                them.clone(),
            ),
            (
                format!(
                    "is a land in addition to its other types and is {article} {name} in addition to its other {kind}"
                ),
                it.clone(),
            ),
            (
                format!(
                    "is {article} {name} in addition to its other {kind} and is a land in addition to its other types"
                ),
                it.clone(),
            ),
        ];
        for (from, into) in said {
            found.extend(
                texts
                    .iter()
                    .filter(|t| t.contains(&from))
                    .map(|t| t.replace(&from, &into)),
            );
        }
    }
    texts.extend(found);
}
