//! Emblems (CR 114): "you get an emblem with "..."" and "target opponent gets an emblem with "..."", the emblem's
//! ability read in its quotes as the ability of a permanent, and written back.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

use super::render;
use super::tokens::Reader;
use super::{abilities, statics};

/// After the subject and "get"/"gets": "an emblem with" and one ability in quotes (a static, triggered or activated
/// ability; "this emblem" is the emblem itself).
pub(super) fn w6b_read(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    r.words("an emblem with").then_some(())?;
    let mut found = vec![r.quoted(w6b_ability)?];
    while let Some(next) = r.attempt(|r| {
        r.word("and").then_some(())?;
        r.quoted(w6b_ability)
    }) {
        found.push(next);
    }
    Some(Effect::CreateEmblem {
        who,
        abilities: found,
    })
}

/// One ability of the emblem, the whole text in its quotes.
fn w6b_ability(r: &mut Reader<'_>) -> Option<Ability> {
    r.attempt(statics::ability)
        .or_else(|| r.attempt(abilities::triggered))
        .or_else(|| r.attempt(abilities::activated))
}

/// "you get an emblem with "..."": the subject, its verb and each ability in quotes.
pub(super) fn w6b_text(subject: &str, you: bool, abilities: &[Ability]) -> Option<String> {
    let quoted = abilities
        .iter()
        .map(|ability| render::ability(ability).map(|text| format!("\"{text}\"")))
        .collect::<Option<Vec<_>>>()?;
    (!quoted.is_empty()).then_some(())?;
    let verb = if you { "get" } else { "gets" };
    Some(format!(
        "{subject} {verb} an emblem with {}",
        quoted.join(" and ")
    ))
}
