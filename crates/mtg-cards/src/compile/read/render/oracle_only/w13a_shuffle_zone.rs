//! A whole zone of the controller shuffled into the library: "{T}, Exile this artifact: Shuffle your graveyard into
//! your library." The head comes from the renderers and the sentence names the zone.

use serde_json::{Value, json};

use super::w4d_parts::{heads, joined, parts};
use crate::ability::Ability;

/// The zones a player shuffles into their library and their words.
const W13A_ZONES: [(&str, &str); 2] = [("Graveyard", "graveyard"), ("Hand", "hand")];

/// Adds the wordings of an ability whose one effect shuffles every card of one of the controller's zones into their
/// library.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((value, effects)) = parts(ability) else {
        return;
    };
    let [effect] = effects.as_slice() else {
        return;
    };
    let Some(word) = w13a_zone(effect) else {
        return;
    };
    let sentence = format!("Shuffle your {word} into your library.");
    for head in heads(&value) {
        texts.push(joined(&head, &sentence));
    }
}

/// The word of the zone a move shuffles into the controller's library, when it moves all their cards in it.
fn w13a_zone(effect: &Value) -> Option<&'static str> {
    let moved = effect.get("Move")?;
    if moved.as_object()?.len() != 2
        || moved.get("to")? != &json!({"Library": {"place": "Shuffled"}})
    {
        return None;
    }
    let preds = moved.get("what")?.get("Each")?.as_array()?;
    let [first, second] = preds.as_slice() else {
        return None;
    };
    let (zone, owner) = if first.get("In").is_some() {
        (first, second)
    } else {
        (second, first)
    };
    if owner != &json!({"OwnedBy": "You"}) {
        return None;
    }
    let zone = zone.get("In")?.as_str()?;
    W13A_ZONES
        .iter()
        .find(|(name, _)| *name == zone)
        .map(|(_, word)| *word)
}
