//! The defending player exiles permanents of their own choice: "defending player exiles two permanents they control".

use super::super::{Amount, Bounds, Effect, Form, Pred, Ref, Selector, count_word, phrase_text};

/// The number a choice takes exactly, when it is a fixed number of two or more.
pub(super) fn exactly(count: &Bounds) -> Option<u32> {
    match (count.fixed_min(), &count.max) {
        (Some(least @ 2..), Some(Amount::Fixed(most))) if i64::from(least) == i64::from(*most) => {
            Some(least)
        }
        _ => None,
    }
}

/// The sentence for the choice of permanents the defending player controls and their exile, and the two effects it
/// covers.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Choose {
            who: Ref::Defending,
            from: Ref::Each(Selector(preds)),
            count,
            bind,
            rest_as: None,
        },
        Effect::Exile {
            what: Ref::Bound(name),
            bind: None,
            duration: None,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    (bind == name).then_some(())?;
    let own = Pred::ControlledBy(Ref::Defending);
    let rest: Vec<Pred> = preds.iter().filter(|pred| **pred != own).cloned().collect();
    (rest.len() + 1 == preds.len()).then_some(())?;
    let group = if rest.is_empty() {
        "permanents".to_owned()
    } else {
        phrase_text(&Selector(rest), Form::All)?
    };
    let number = count_word(exactly(count)?)?;
    Some((
        format!("defending player exiles {number} {group} they control"),
        2,
    ))
}
