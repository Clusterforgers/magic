//! A replacement of the tokens any player creates: "If one or more tokens would be created, twice that many of those
//! tokens are created instead." The renderers word the tokens of the controller, "under your control".

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;

/// Adds the wordings of a replacement of the tokens created under any player's control.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let creation = value
            .get_mut("Replacement")?
            .get_mut("when")?
            .get_mut("TokenCreation")?
            .as_object_mut()?;
        (creation.get("controller")? == "EachPlayer").then_some(())?;
        creation.remove("controller").map(|_| ())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .map(|text| text.replacen(" under your control", "", 1))
        .collect();
    texts.extend(said);
}
