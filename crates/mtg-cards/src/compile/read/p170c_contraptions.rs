//! The Contraption assembly wordings (Unstable): "assemble a Contraption" and "assemble two Contraptions"; the crank
//! trigger "whenever you crank this Contraption" is read with the other player events (`triggers.rs`).

use super::amounts::{count_word, number_word};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// The rest of "assemble a Contraption" or "assemble two Contraptions" after the verb, for the player `who`.
pub(super) fn p170c_assemble(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    let count = number_word(r.take_word()?)?;
    let noun = if count == 1 {
        "Contraption"
    } else {
        "Contraptions"
    };
    (r.name() == Some(noun)).then_some(())?;
    Some(Effect::P170bAssemble {
        who,
        count: Amount::Fixed(i32::try_from(count).ok()?),
    })
}

/// An assembly written back, for "you"; another player is not written.
pub(super) fn p170c_assemble_text(effect: &Effect) -> Option<String> {
    match effect {
        Effect::P170bAssemble {
            who: Ref::You,
            count: Amount::Fixed(1),
        } => Some("assemble a Contraption".to_owned()),
        Effect::P170bAssemble {
            who: Ref::You,
            count: Amount::Fixed(count),
        } => Some(format!(
            "assemble {} Contraptions",
            count_word(u32::try_from(*count).ok()?)?
        )),
        _ => None,
    }
}
