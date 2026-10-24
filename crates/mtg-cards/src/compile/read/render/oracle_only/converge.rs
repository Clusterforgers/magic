//! Damage by the colors of mana spent to cast the spell: "~ deals X damage to each creature, where X is the number of
//! colors of mana spent to cast this spell."

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::json;

/// Adds the wordings of a spell that deals damage by the number of colors spent to cast it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let effects = value.get_mut("Spell")?.get_mut("effects")?.as_array_mut()?;
        let [effect] = effects.as_mut_slice() else {
            return None;
        };
        let amount = effect.get_mut("DealDamage")?.get_mut("amount")?;
        (*amount == "ColorsSpent").then_some(())?;
        *amount = json!(1);
        Some(())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains(" deals 1 damage "))
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| {
            format!(
                "{}, where X is the number of colors of mana spent to cast this spell.",
                text.replacen(" deals 1 damage ", " deals X damage ", 1)
            )
        })
        .collect();
    texts.extend(said);
}
