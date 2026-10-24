//! A pick among the cards looked at: "Put one of those cards into your hand and the rest on the bottom of your library
//! in any order." The renderers write "one of them".

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings that call the cards looked at "those cards" in the pick.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !ability
        .effects()
        .iter()
        .any(|effect| matches!(effect, Effect::LookAtTop { .. }))
    {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("Put one of them "))
        .map(|text| text.replacen("Put one of them ", "Put one of those cards ", 1))
        .collect();
    texts.extend(found);
}
