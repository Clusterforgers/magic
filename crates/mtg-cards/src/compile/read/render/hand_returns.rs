//! Cards of the graveyard returned to the hand: "Return all legendary creature cards from your graveyard to your hand.",
//! and a mill with the optional return after it ("Mill three cards, then you may return a creature or land card from
//! your graveyard to your hand.").

use super::{Destination, Effect, Form, Writer, article, phrase_text};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;

/// How many a choice takes, worded: "a land card" (exactly one, or "you may" for none or one), "two land cards", "up to
/// two land cards", "any number of land cards".
pub(super) struct Quantity {
    /// "you may " or nothing, before the verb.
    pub lead: &'static str,
    /// The cards or permanents, with their number.
    pub noun: String,
    /// More than one: "their owner's hand" for "its owner's hand".
    pub plural: bool,
}

/// The words for the count of a choice from the selector; any other count is not worded here.
pub(super) fn quantity(count: &Bounds, from: &Selector) -> Option<Quantity> {
    let one = |lead| {
        let phrase = phrase_text(from, Form::Subject)?;
        let noun = if phrase.starts_with("another ") {
            phrase
        } else {
            format!("{} {phrase}", article(&phrase))
        };
        Some(Quantity {
            lead,
            noun,
            plural: false,
        })
    };
    if count.is_one() {
        return one("");
    }
    if *count == Bounds::up_to(Some(Amount::Fixed(1))) {
        return one("you may ");
    }
    let many = |lead_in: &str| {
        Some(Quantity {
            lead: "",
            noun: format!("{lead_in}{}", phrase_text(from, Form::All)?),
            plural: true,
        })
    };
    let number = |count: i32| super::count_word(u32::try_from(count).ok()?);
    match (&count.min, &count.max) {
        (Amount::Fixed(0), None) => many("any number of "),
        (Amount::Fixed(0), Some(Amount::Fixed(most))) if *most >= 2 => {
            many(&format!("up to {} ", number(*most)?))
        }
        (Amount::Fixed(least), Some(Amount::Fixed(most))) if least == most && *least >= 2 => {
            many(&format!("{} ", number(*least)?))
        }
        _ => None,
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
            differ: None,
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
    let q = quantity(count, from)?;
    Some(format!("{}return {} to your hand", q.lead, q.noun))
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
