//! Control of the permanent an Aura enchants: "You control enchanted artifact creature." The noun repeats the Enchant
//! keyword, which the keyword reader checks, so each permanent kind an Aura can enchant is a wording.

use serde_json::json;

use crate::ability::Ability;

/// The kinds of permanents an Aura that steals them enchants.
const W16A_KINDS: [&str; 7] = [
    "creature",
    "artifact",
    "artifact creature",
    "enchantment",
    "land",
    "planeswalker",
    "permanent",
];

/// Adds the wordings of a static ability that gives you control of the enchanted permanent.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(inner) = value.get("Static") else {
        return;
    };
    let plain = inner.get("affected") == Some(&json!("Attached"))
        && inner.get("mods") == Some(&json!([{"SetController": "You"}]))
        && inner
            .get("condition")
            .is_none_or(serde_json::Value::is_null);
    if plain {
        texts.extend(
            W16A_KINDS
                .iter()
                .map(|kind| format!("You control enchanted {kind}.")),
        );
    }
}
