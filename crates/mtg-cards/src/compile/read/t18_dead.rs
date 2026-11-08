//! The card of a permanent that died, named by the event ("Triggering") or as the card now in the graveyard: after a
//! "dies" trigger the engine follows the event's card into the graveyard, so a move of it is the move of "that card".

use serde_json::{Value, json};

use crate::ability::Ability;
use super::effects::other_dead_card;

/// Whether the trigger pattern is a permanent going from the battlefield to a graveyard.
fn is_death(when: &Value) -> bool {
    let Some(change) = when.get("ZoneChange") else {
        return false;
    };
    change.get("from") == Some(&json!("Battlefield")) && change.get("to") == Some(&json!("Graveyard"))
}

/// The ability of a death trigger with each move of the triggering object written as a move of the card in the
/// graveyard; `None` for any other ability or when no move names the triggering object.
pub(super) fn t18_dead_card(ability: &Ability) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let body = value.get_mut("Triggered")?;
    is_death(body.get("when")?).then_some(())?;
    let card = serde_json::to_value(other_dead_card()).ok()?;
    let mut changed = false;
    for effect in body.get_mut("effects")?.as_array_mut()? {
        if let Some(moved) = effect.get_mut("Move")
            && moved.get("what") == Some(&json!("Triggering"))
        {
            moved["what"] = card.clone();
            changed = true;
        }
    }
    changed.then_some(())?;
    serde_json::from_value(value).ok()
}
