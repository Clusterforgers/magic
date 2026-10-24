//! A choice that makes a target player do something: "you may have target player lose 3 life". The renderers write
//! "you may target player loses 3 life".

use crate::ability::Ability;
use crate::effect::Effect;

/// The players a choice can name.
const PLAYERS: [&str; 2] = ["target player", "target opponent"];

/// Adds the wordings of a choice made for a target player, with "have" and the verb in its base form.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !ability
        .effects()
        .iter()
        .any(|effect| matches!(effect, Effect::May { .. }))
    {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| have(text)).collect();
    texts.extend(found);
}

/// The text with the first "you may target player loses" worded "you may have target player lose".
fn have(text: &str) -> Option<String> {
    PLAYERS.iter().find_map(|player| {
        let from = format!("you may {player} ");
        let at = text.find(&from)?;
        let rest = &text[at + from.len()..];
        let (verb, tail) = rest.split_once(' ')?;
        let base = verb.strip_suffix('s')?;
        Some(format!(
            "{}you may have {player} {base} {tail}",
            &text[..at]
        ))
    })
}
