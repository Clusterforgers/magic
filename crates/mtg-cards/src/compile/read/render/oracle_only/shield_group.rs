//! Combat damage prevented for a group: "Whenever this creature attacks, prevent all combat damage that would be dealt
//! this turn to Dogs you control." The renderers word the prevention for a player.

use super::super::variants;
use super::twin_json::twin;
use crate::ability::Ability;
use serde_json::json;

/// Adds the wordings of a prevention of combat damage to the creatures of one subtype you control.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut group = None;
    let plain = twin(ability, |value| {
        let effects = value
            .get_mut("Triggered")?
            .get_mut("effects")?
            .as_array_mut()?;
        let [effect] = effects.as_mut_slice() else {
            return None;
        };
        let damage = effect
            .get_mut("Shield")?
            .get_mut("when")?
            .get_mut("Damage")?;
        (damage.get("combat")? == true).then_some(())?;
        let selector = damage.get("to")?.get("Each")?.as_array()?;
        group = match selector.as_slice() {
            [subtype, control] if *control == json!({"ControlledBy": "You"}) => {
                subtype.get("Subtype")?.as_str().map(str::to_owned)
            }
            _ => None,
        };
        group.as_ref()?;
        *damage.get_mut("to")? = json!("You");
        Some(())
    });
    let (Some(plain), Some(group)) = (plain, group) else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains(" that would be dealt to you this turn"))
        .map(|text| {
            text.replacen(
                " that would be dealt to you this turn",
                &format!(" that would be dealt this turn to {group}s you control"),
                1,
            )
        })
        .collect();
    texts.extend(said);
}
