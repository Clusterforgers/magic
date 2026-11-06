//! Two printed wordings of rules the renderers write another way: "permanent cards" for the six permanent types listed
//! (Descend), and "Excess damage is dealt to that creature's controller instead." after damage to a target creature.

use crate::ability::Ability;

/// The six permanent types as the renderers list them, singular and plural, and the one noun the cards print.
const FIX9_PERMANENT: [(&str, &str); 2] = [
    (
        "artifact, battle, creature, enchantment, land, or planeswalker cards",
        "permanent cards",
    ),
    (
        "artifact, battle, creature, enchantment, land, or planeswalker card",
        "permanent card",
    ),
];

/// The rule of damage whose excess goes to the controller of the creature targeted first.
const FIX9_EXCESS: &str = r#""excess":{"to":{"ControllerOf":{"Target":0}}}"#;

/// The sentence a card prints after that damage.
const FIX9_EXCESS_TEXT: &str = " Excess damage is dealt to that creature's controller instead.";

/// Adds the wordings with "permanent card" for the listed types, and with the excess sentence for damage that has it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(FIX9_PERMANENT[1].0))
        .map(|text| {
            FIX9_PERMANENT
                .iter()
                .fold(text.clone(), |text, (from, to)| text.replace(from, to))
        })
        .collect();
    let excess = serde_json::to_string(ability).is_ok_and(|json| json.contains(FIX9_EXCESS));
    if excess {
        said.extend(
            texts
                .iter()
                .filter(|text| text.contains("damage to target creature") && text.ends_with('.'))
                .map(|text| format!("{text}{FIX9_EXCESS_TEXT}")),
        );
    }
    texts.extend(said);
}
