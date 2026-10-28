//! Shared parts of the r3w9 writers: the noun phrase of a selector ("Merfolk you control") and of a target ("target
//! artifact card"), each taken from a known-good stand-in spell whose wording names it.

use serde_json::{Value, json};

use super::w4d_parts::wordings;

/// The noun phrases of the objects a selector picks, without an article: "Merfolk you control", "Gate you control".
pub(super) fn w5b_noun(selector: &Value) -> Vec<String> {
    let spell = json!({"Spell": {
        "targets": [{"what": {"Object": selector}}],
        "effects": [{"Destroy": {"what": {"Target": 0}}}],
    }});
    wordings(spell)
        .iter()
        .filter_map(|text| text.strip_prefix("Destroy target ")?.strip_suffix('.'))
        .map(str::to_owned)
        .collect()
}

/// The phrases of the first target, a card in your graveyard, with "target" in front: "target artifact card".
pub(super) fn w5b_graveyard_target(targets: &Value) -> Vec<String> {
    let spell = json!({"Spell": {
        "targets": targets,
        "effects": [{"Move": {"what": {"Target": 0}, "to": "Hand"}}],
    }});
    wordings(spell)
        .iter()
        .filter_map(|text| {
            text.strip_prefix("Return ")?
                .strip_suffix(" from your graveyard to your hand.")
        })
        .filter(|phrase| phrase.starts_with("target "))
        .map(str::to_owned)
        .collect()
}

/// The words for a whole number of things: "one", "seven".
pub(super) fn w5b_number(value: &Value) -> Option<&'static str> {
    let number = u32::try_from(value.as_u64()?).ok()?;
    super::super::super::amounts::count_word(number)
}
