//! An Aura that makes the permanent another kind: "Enchanted permanent is a colorless land with "{T}: Add {C}" and loses
//! all other card types and abilities." and "Enchanted permanent is a colorless Clue artifact with "{2}, Sacrifice this
//! artifact: Draw a card" and loses all other abilities."

use super::super::variants;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::Ref;

/// Adds the wording of a static ability that sets the type and the color, takes the abilities and gives one.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected: Ref::Attached,
        mods,
        condition: None,
        defining: false,
        ..
    } = ability
    else {
        return;
    };
    let (types, subtype, granted) = match mods.as_slice() {
        [
            Modification::SetTypes(types),
            Modification::SetColors(colors),
            Modification::LoseAbilities,
            Modification::GrantAbility(granted),
        ] if colors.is_empty() => (types, None, granted),
        [
            Modification::SetTypes(types),
            Modification::AddSubtype(subtype),
            Modification::SetColors(colors),
            Modification::LoseAbilities,
            Modification::GrantAbility(granted),
        ] if colors.is_empty() => (types, Some(subtype), granted),
        _ => return,
    };
    let [kind] = types.as_slice() else {
        return;
    };
    let Some(given) = variants(granted)
        .first()
        .map(|text| text.trim_end_matches('.').to_owned())
    else {
        return;
    };
    let kind = format!("{kind:?}").to_lowercase();
    let (named, lost) = match subtype {
        Some(subtype) => (format!("{} {kind}", subtype.0), "abilities"),
        None => (kind, "card types and abilities"),
    };
    texts.push(format!(
        "Enchanted permanent is a colorless {named} with \"{given}\" and loses all other {lost}."
    ));
}
