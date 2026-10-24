//! A card put onto the battlefield from the hand: "put a land card from your hand onto the battlefield tapped" (the
//! choice of one card of the hand, bound, and the move of it), and a card of the graveyard returned to it: "return a
//! creature card from your graveyard to the battlefield with a finality counter on it".

use super::{Destination, Effect, Form, Writer, article, phrase_text};
use crate::compile::read::counters;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::value::Amount;

/// "put a land card from your hand onto the battlefield", "... onto the battlefield tapped", for the choice of one
/// card of the hand and the move of what was chosen.
pub(super) fn put_onto_text(choose: &Effect, moved: &Effect) -> Option<String> {
    let (
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind,
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(name),
            to:
                Destination::Battlefield {
                    tapped,
                    under: None,
                    transformed: false,
                    counters,
                },
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != name
        || !count.is_one()
        || !counters.is_empty()
        || !from
            .0
            .starts_with(&[Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)])
    {
        return None;
    }
    let phrase = phrase_text(from, Form::Subject)?;
    let tapped = if *tapped { " tapped" } else { "" };
    Some(format!(
        "put {} {phrase} onto the battlefield{tapped}",
        article(&phrase)
    ))
}

impl Writer<'_> {
    /// A step on the library and the return of a card of the graveyard as one sentence, which no joint splits:
    /// "surveil 3, then return a creature card from your graveyard to the battlefield".
    pub(super) fn library_then_return(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            first @ (Effect::Surveil { .. } | Effect::Scry { .. } | Effect::Mill { .. }),
            choose @ Effect::Choose { .. },
            moved @ Effect::Move { .. },
            ..,
        ] = effects
        else {
            return None;
        };
        let returned = returned_to_battlefield_text(choose, moved)?;
        Some((format!("{}, then {returned}", self.effect(first)?), 3))
    }
}

/// "return a creature card from your graveyard to the battlefield", "... tapped", "... with a finality counter on it",
/// for the choice of one card of the graveyard and the move of what was chosen.
pub(super) fn returned_to_battlefield_text(choose: &Effect, moved: &Effect) -> Option<String> {
    let (
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind,
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(name),
            to:
                Destination::Battlefield {
                    tapped,
                    under: None,
                    transformed: false,
                    counters,
                },
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != name
        || !count.is_one()
        || !from
            .0
            .starts_with(&[Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)])
    {
        return None;
    }
    let phrase = phrase_text(from, Form::Subject)?;
    let tapped = if *tapped { " tapped" } else { "" };
    let with = match counters.as_slice() {
        [] => String::new(),
        [(kind, number)] => {
            let counters = counters::text(kind, &Amount::Fixed(i32::try_from(*number).ok()?))?;
            format!(" with {counters} on it")
        }
        _ => return None,
    };
    Some(format!(
        "return {} {phrase} to the battlefield{tapped}{with}",
        article(&phrase)
    ))
}
