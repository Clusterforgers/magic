//! A trigger on a spell of the color the permanent chose: "Whenever a player casts a spell of the chosen color, you may
//! gain 1 life."

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::json;

/// Adds the wordings of a trigger on a spell of the chosen color.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let cast = value
            .get_mut("Triggered")?
            .get_mut("when")?
            .get_mut("SpellCast")?;
        (cast.get("what")? == &json!(["ChosenColor"])).then_some(())?;
        cast.as_object_mut()?.remove("what").map(|_| ())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains("casts a spell, "))
        .map(|text| text.replacen("casts a spell, ", "casts a spell of the chosen color, ", 1))
        .collect();
    texts.extend(said);
}
