//! A choice of some objects and the destruction of the rest: "Choose up to one creature. Destroy the rest."

use super::super::{Bounds, Effect, Form, Ref, Selector, article, phrase_text};
use crate::value::Amount;

/// The sentences for the choice of at most one object and the destruction of all the others, and the two effects they
/// cover.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(Selector(preds)),
            count,
            bind,
            rest_as: Some(rest),
        },
        Effect::Destroy {
            what: Ref::Bound(left),
            regenerate: true,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    (left == rest && bind != rest).then_some(())?;
    let kind = phrase_text(&Selector(preds.clone()), Form::Subject)?;
    let chosen = if *count == Bounds::up_to(Some(Amount::Fixed(1))) {
        format!("up to one {kind}")
    } else if count.is_one() {
        format!("{} {kind}", article(&kind))
    } else {
        return None;
    };
    Some((format!("choose {chosen}. Destroy the rest"), 2))
}
