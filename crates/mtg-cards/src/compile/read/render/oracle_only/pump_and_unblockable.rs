//! A pump with evasion for the turn: "this creature gets +1/+0 until end of turn and can't be blocked this turn." The
//! renderers have no wording for the two changes in one effect.

use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;

/// Adds the wordings of a pump until end of turn that also makes the creature unblockable.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let plain = twin(ability, |value| {
        let effects = inner_mut(value)?.get_mut("effects")?.as_array_mut()?;
        let [effect] = effects.as_mut_slice() else {
            return None;
        };
        let change = effect.get_mut("Continuous")?;
        (change.get("duration")? == "UntilEndOfTurn").then_some(())?;
        let mods = change.get_mut("mods")?.as_array_mut()?;
        let at = mods.iter().position(|one| one == "CantBeBlocked")?;
        (mods.len() == 2 && mods[1 - at].get("ModifyPT").is_some()).then_some(())?;
        mods.remove(at);
        Some(())
    });
    let Some(plain) = plain else {
        return;
    };
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains(" gets "))
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| format!("{text} and can't be blocked this turn."))
        .collect();
    texts.extend(said);
}
