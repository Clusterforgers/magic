//! The owner of a target as the one who acts: "Destroy target creature. Its owner gains 4 life." The ability is written
//! with the controller of the target in that place and the words are swapped.

use serde_json::json;

use super::w4d_parts::{swapped, wordings};
use crate::ability::Ability;

/// The words for the controller of a target and for its owner.
const W16A_SWAPS: [(&str, &str); 4] = [
    ("Its controller ", "Its owner "),
    ("its controller ", "its owner "),
    ("That creature's controller ", "That creature's owner "),
    ("that creature's controller ", "that creature's owner "),
];

/// Adds the wordings of an ability in which the owner of its first target acts.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let controller = json!({"ControllerOf": {"Target": 0}});
    if value.to_string().contains("ControllerOf")
        || !swapped(&mut value, &json!({"OwnerOf": {"Target": 0}}), &controller)
    {
        return;
    }
    for text in wordings(value) {
        let said: Vec<&(&str, &str)> = W16A_SWAPS
            .iter()
            .filter(|(from, _)| text.contains(from))
            .collect();
        if let [(from, to)] = said.as_slice()
            && text.matches(from).count() == 1
        {
            texts.push(text.replace(from, to));
        }
    }
}
