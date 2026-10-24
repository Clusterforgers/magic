//! An untap of lands the oracle leaves unqualified: "Untap up to two lands." The renderers add "you control".

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings of an untap of up to some lands without the owner named.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !ability
        .effects()
        .iter()
        .any(|effect| matches!(effect, Effect::Untap { .. }))
    {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let at = text.find("ntap up to ")?;
            let (head, tail) = text.split_at(at);
            let tail = tail.replacen(" lands you control", " lands", 1);
            (tail != text[at..]).then(|| format!("{head}{tail}"))
        })
        .collect();
    texts.extend(said);
}
