//! A spell that ends with no maximum hand size: "Return all cards from your graveyard to your hand. Exile ~. You have no
//! maximum hand size for the rest of the game."

use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;
use serde_json::json;

/// Adds the wordings of a last step that lifts the maximum hand size for the rest of the game.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let effects = inner_mut(value)?.get_mut("effects")?.as_array_mut()?;
        let rule = json!({"Rule": {"rule": {"NoMaxHandSize": "You"}, "duration": "Forever"}});
        (effects.len() > 1 && effects.last()? == &rule).then_some(())?;
        effects.pop().map(|_| ())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .map(|text| format!("{text} You have no maximum hand size for the rest of the game."))
        .collect();
    texts.extend(said);
}
