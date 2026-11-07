//! Cards of the graveyard returned to the hand: "Return all legendary creature cards from your graveyard to your hand.",
//! and a mill with the optional return after it ("Mill three cards, then you may return a creature or land card from
//! your graveyard to your hand.").

use super::{Destination, Effect, Form, Writer, article, phrase_text};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;

/// How a choice of one card is worded: nothing before the verb for exactly one ("return a land card"), "you may " for
/// none or one ("you may return a land card"); any other count is not worded here.
pub(super) fn may_choice(count: &Bounds) -> Option<&'static str> {
    if count.is_one() {
        Some("")
    } else if *count == Bounds::up_to(Some(Amount::Fixed(1))) {
        Some("you may ")
    } else {
        None
    }
}

/// Whether the selector names cards of the graveyard of the player.
fn from_your_graveyard(selector: &Selector) -> bool {
    in_your(selector, ZoneKind::Graveyard)
}

/// Whether the selector names cards of the given zone of the player, wherever its words stand in the list.
pub(super) fn in_your(selector: &Selector, zone: ZoneKind) -> bool {
    selector.0.contains(&Pred::In(zone)) && selector.0.contains(&Pred::OwnedBy(Ref::You))
}

/// "return all legendary creature cards from your graveyard to your hand".
pub(super) fn all_text(effect: &Effect) -> Option<String> {
    let Effect::Move {
        what: Ref::Each(selector),
        to: Destination::Hand,
        ..
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
pub(super) fn one_text(choose: &Effect, moved: &Effect) -> Option<String> {
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
            ..
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != name || !from_your_graveyard(from) {
        return None;
    }
    let may = may_choice(count)?;
    let phrase = phrase_text(from, Form::Subject)?;
    Some(format!(
        "{may}return {} {phrase} to your hand",
        article(&phrase)
    ))
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
                (!returned.starts_with("you may ")).then_some(())?;
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
