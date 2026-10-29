//! An exile of the cards of a target player's graveyard said as that player's action: "Target player exiles all land
//! cards from their graveyard." The ability is written as the controller's own exile from their graveyard and put in
//! the third person.

use serde_json::{Value, json};

use super::w4d_parts::{capitalized, heads, joined, swapped, third_person, wordings};
use crate::ability::Ability;

/// Adds the wordings of an ability whose one effect exiles cards of the graveyard of its one target, a player.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(inner) = value.as_object().and_then(|map| map.values().next()) else {
        return;
    };
    let one_player = inner
        .get("targets")
        .and_then(Value::as_array)
        .is_some_and(|targets| targets.len() == 1 && targets[0] == json!({"what": "Player"}));
    let Some([effect]) = inner
        .get("effects")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return;
    };
    if !one_player || !w12a_graveyard_exile(effect) {
        return;
    }
    let mut own = effect.clone();
    if !swapped(
        &mut own,
        &json!({"OwnedBy": {"Target": 0}}),
        &json!({"OwnedBy": "You"}),
    ) {
        return;
    }
    let sentences: Vec<String> = wordings(json!({"Spell": {"effects": [own]}}))
        .iter()
        .filter(|text| text.contains("your graveyard"))
        .filter_map(|text| third_person(text, "target player"))
        .collect();
    for head in heads(&value) {
        texts.extend(
            sentences
                .iter()
                .map(|sentence| joined(&head, &format!("{}.", capitalized(sentence)))),
        );
    }
}

/// Whether the effect exiles each card of a kind in the graveyard of the first target.
fn w12a_graveyard_exile(effect: &Value) -> bool {
    effect
        .get("Exile")
        .and_then(|exile| exile.get("what"))
        .and_then(|what| what.get("Each"))
        .and_then(Value::as_array)
        .is_some_and(|selector| {
            selector.contains(&json!({"In": "Graveyard"}))
                && selector.contains(&json!({"OwnedBy": {"Target": 0}}))
        })
}
