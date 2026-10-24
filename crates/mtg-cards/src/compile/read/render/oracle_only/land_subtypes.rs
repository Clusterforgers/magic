//! A basic land type given alone: "Each land is a Forest in addition to its other land types." The renderers call every
//! subtype a creature type.

use crate::ability::Ability;
use crate::modify::Modification;

/// The basic land types.
const LAND_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// Adds the wordings of a static ability that adds one basic land type and no card type.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static { mods, .. } = ability else {
        return;
    };
    let [Modification::AddSubtype(subtype)] = mods.as_slice() else {
        return;
    };
    if !LAND_TYPES.contains(&subtype.0.as_str()) {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" other creature types"))
        .map(|text| text.replacen(" other creature types", " other land types", 1))
        .collect();
    texts.extend(said);
}
