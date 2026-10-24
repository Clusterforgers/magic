//! Cards of the graveyard returned to the hand: "Return all legendary creature cards from your graveyard to your hand.",
//! and a mill with the optional return after it ("Mill three cards, then you may return a creature or land card from
//! your graveyard to your hand.").

use super::{Destination, Effect, Form, Writer, article, phrase_text};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// Whether the selector names cards of the graveyard of the player.
fn from_your_graveyard(selector: &Selector) -> bool {
    selector
        .0
        .starts_with(&[Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)])
}

/// "return all legendary creature cards from your graveyard to your hand".
pub(super) fn all_text(effect: &Effect) -> Option<String> {
    let Effect::Move {
        what: Ref::Each(selector),
        to: Destination::Hand,
    } = effect
    else {
        return None;
    };
    from_your_graveyard(selector).then_some(())?;
    Some(format!(
        "return all {} to your hand",
        phrase_text(selector, Form::All)?
    ))
}

/// "return a creature or land card from your graveyard to your hand": the choice of one card and the move of it.
fn one_text(choose: &Effect, moved: &Effect) -> Option<String> {
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
            to: Destination::Hand,
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != name || !count.is_one() || !from_your_graveyard(from) {
        return None;
    }
    let phrase = phrase_text(from, Form::Subject)?;
    Some(format!("return {} {phrase} to your hand", article(&phrase)))
}

impl Writer<'_> {
    /// A mill and the return of a card, which may be optional, as one sentence no joint splits.
    pub(super) fn mill_then_hand(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        match effects {
            [
                first @ Effect::Mill { .. },
                Effect::May {
                    who: Ref::You,
                    effects: inner,
                },
                ..,
            ] => {
                let [choose, moved] = inner.as_slice() else {
                    return None;
                };
                let returned = one_text(choose, moved)?;
                Some((
                    format!("{}, then you may {returned}", self.effect(first)?),
                    2,
                ))
            }
            [
                first @ Effect::Mill { .. },
                choose @ Effect::Choose { .. },
                moved @ Effect::Move { .. },
                ..,
            ] => {
                let returned = one_text(choose, moved)?;
                Some((format!("{}, then {returned}", self.effect(first)?), 3))
            }
            _ => None,
        }
    }
}
