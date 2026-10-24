//! Tokens for each spell the player cast this turn: "At the beginning of your end step, for each spell you've cast this
//! turn, create a 1/2 blue Bird creature token with flying named Storm Crow."

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::json;

/// Adds the wordings of a trigger that creates one token for each spell the controller cast this turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let effects = value
            .get_mut("Triggered")?
            .get_mut("effects")?
            .as_array_mut()?;
        let tally = json!({"Tallied": {"who": "You", "what": "SpellsCast"}});
        let [effect] = effects.as_mut_slice() else {
            return None;
        };
        let create = effect.get_mut("CreateToken")?.as_object_mut()?;
        (create.get("count")? == &tally).then_some(())?;
        create.remove("count").map(|_| ())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.split_once(", "))
        .map(|(event, step)| format!("{event}, for each spell you've cast this turn, {step}"))
        .collect();
    texts.extend(said);
}
