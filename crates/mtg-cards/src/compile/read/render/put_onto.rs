//! A card put onto the battlefield from the hand: "put a land card from your hand onto the battlefield tapped" (the
//! choice of one card of the hand, bound, and the move of it).

use super::{Destination, Effect, Form, article, phrase_text};
use crate::refs::{Pred, Ref, ZoneKind};

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
