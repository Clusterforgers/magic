//! A creature chosen by a trigger that prepares the card when it deals combat damage to a player this combat: "Whenever
//! you attack, target creature gains deathtouch until end of turn. Whenever that creature deals combat damage to a
//! player this combat, this creature becomes prepared."

use serde_json::json;

use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;

/// The second sentence: the delayed trigger on the chosen creature.
const DELAYED: &str = "Whenever that creature deals combat damage to a player this combat, this creature becomes prepared.";

/// Adds the wording of an ability that binds its target and waits for it to hurt a player, from the wordings of the
/// ability without those two steps.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(plain) = twin(ability, |value| {
        let effects = inner_mut(value)?.get_mut("effects")?.as_array_mut()?;
        let delayed = effects.get(2)?;
        let shaped = effects.len() == 3
            && effects[1].pointer("/Choose/from") == Some(&json!({"Target": 0}))
            && delayed.pointer("/DelayedTrigger/once") == Some(&json!(false))
            && delayed.pointer("/DelayedTrigger/when/DealsDamage/to") == Some(&json!("EachPlayer"))
            && delayed.pointer("/DelayedTrigger/when/DealsDamage/combat") == Some(&json!(true))
            && delayed.pointer("/DelayedTrigger/effects")
                == Some(&json!([{"Prepare": {"what": "This"}}]));
        shaped.then(|| effects.truncate(1))
    }) else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .map(|text| format!("{text} {DELAYED}"))
        .collect();
    texts.extend(said);
}
