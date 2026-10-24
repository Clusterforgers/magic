//! The defending player mills cards: "Whenever this creature attacks, defending player mills ten cards."

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;

/// Adds the wordings of a trigger whose mill is done by the defending player.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let effects = value
            .get_mut("Triggered")?
            .get_mut("effects")?
            .as_array_mut()?;
        let [effect] = effects.as_mut_slice() else {
            return None;
        };
        let who = effect.get_mut("Mill")?.get_mut("who")?;
        (*who == "Defending").then_some(())?;
        *who = "Opponents".into();
        Some(())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains("each opponent mills "))
        .map(|text| text.replacen("each opponent mills ", "defending player mills ", 1))
        .collect();
    texts.extend(said);
}
