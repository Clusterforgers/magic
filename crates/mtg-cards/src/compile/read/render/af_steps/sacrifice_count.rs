//! Players who sacrifice several permanents of their choice: "Each player sacrifices two creatures of their choice."

use super::super::{Amount, Effect, Form, Ref, count_word, phrase_text};

/// The sentence for a sacrifice of two or more permanents by every player or every opponent, and the one effect it
/// covers.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Sacrifice {
            who,
            filter,
            count: Amount::Fixed(number),
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let who = match who {
        Ref::EachPlayer => "each player",
        Ref::Opponents => "each opponent",
        _ => return None,
    };
    let number = count_word(u32::try_from(*number).ok().filter(|number| *number >= 2)?)?;
    let group = phrase_text(filter, Form::All)?;
    Some((
        format!("{who} sacrifices {number} {group} of their choice"),
        1,
    ))
}
