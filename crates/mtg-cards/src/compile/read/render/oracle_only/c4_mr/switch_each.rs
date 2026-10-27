//! A switch of power and toughness for a group: "Switch each creature's power and toughness until end of turn." The
//! renderers write a size change of the group in its place.

use super::twin_variants;
use crate::ability::Ability;

/// The switch of every creature, and the size change of every creature the renderers write instead.
const SWITCH: &str = r#"{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["SwitchPT"],"duration":"UntilEndOfTurn"}}"#;
const PLAIN: &str = r#"{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}"#;

/// Adds the wordings of an ability that switches the power and toughness of every creature.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(ability, SWITCH, PLAIN)
        .iter()
        .filter(|text| text.contains("All creatures get +1/+1 until end of turn."))
        .map(|text| {
            text.replacen(
                "All creatures get +1/+1 until end of turn.",
                "Switch each creature's power and toughness until end of turn.",
                1,
            )
        })
        .collect();
    texts.extend(said);
}
