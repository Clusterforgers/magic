//! A choice of permanents that are not targets, and the untap of what was chosen: "Untap up to three lands."

use super::{Effect, Form, Ref, count_word, phrase_text};
use crate::value::Amount;

/// "untap up to three lands": a choice from none to a few permanents, then their untap; the number of effects it takes.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(selector),
            count,
            bind,
            rest_as: None,
        },
        Effect::Untap {
            what: Ref::Bound(chosen),
        },
        ..,
    ] = effects
    else {
        return None;
    };
    (bind == chosen && count.min == Amount::Fixed(0)).then_some(())?;
    let Some(Amount::Fixed(most @ 2..)) = count.max else {
        return None;
    };
    let number = count_word(u32::try_from(most).ok()?)?;
    Some((
        format!("untap up to {number} {}", phrase_text(selector, Form::All)?),
        2,
    ))
}
