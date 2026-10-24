//! A card that returns an object and attaches itself to it: "When this Equipment enters, return target creature card with
//! mana value 3 or less from your graveyard to the battlefield and attach this Equipment to it."

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::json;

/// Adds the wordings of a trigger whose last step attaches the card to the object the step before moved.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let effects = value
            .get_mut("Triggered")?
            .get_mut("effects")?
            .as_array_mut()?;
        let attach = json!({"Attach": {"what": "This", "to": {"Target": 0}}});
        (effects.len() == 2 && effects[1] == attach).then_some(())?;
        effects.pop().map(|_| ())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains(" to the battlefield"))
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| format!("{text} and attach ~ to it."))
        .collect();
    texts.extend(said);
}
