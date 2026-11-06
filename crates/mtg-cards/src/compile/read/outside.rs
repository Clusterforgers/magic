//! A card from outside the game (CR 400.10, the wishes): "reveal a sorcery card you own from outside the game and put it
//! into your hand". A Commander game has no sideboard, so the search of `ZoneKind::Outside` finds nothing; the sentence is
//! the same one the card prints.

use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, StatKind};

use super::library::filter_text;
use super::triggers::article;

/// What the cards must be, in the card's words: "a creature or land card you own", "a card you own named X". `None` for a
/// filter the wishes do not use.
fn what(filter: &Selector) -> Option<String> {
    let mut owned = false;
    let mut name = None;
    let mut rest = Vec::new();
    for pred in &filter.0 {
        match pred {
            Pred::OwnedBy(Ref::You) => owned = true,
            Pred::Named(text) => name = Some(text.clone()),
            other => rest.push(other.clone()),
        }
    }
    let kind = match rest.as_slice() {
        [
            Pred::Compare {
                stat: StatKind::Colors,
                op: Cmp::Ge,
                to: Amount::Fixed(2),
                ..
            },
        ] => "multicolored card".to_owned(),
        other => filter_text(&Selector(other.to_vec()), false)?,
    };
    let mut text = format!("{} {kind}", article(&kind));
    if owned {
        text += " you own";
    }
    if let Some(name) = name {
        text += &format!(" named {name}");
    }
    Some(text)
}

/// The search outside the game and the move of what it finds into the hand, and how many effects they are: "reveal a
/// sorcery card you own from outside the game and put it into your hand", or "put a card you own from outside the game
/// into your hand" when nothing is revealed.
pub(super) fn sequence(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Search {
            who: Ref::You,
            of: None,
            zone: ZoneKind::Outside,
            also,
            filter,
            count,
            reveal,
            bind,
        },
        Effect::Move {
            what: Ref::Bound(moved),
            to: Destination::Hand,
            ..
        },
        ..,
    ] = effects
    else {
        return None;
    };
    if !also.is_empty() || !count.is_one() || moved != bind {
        return None;
    }
    let cards = what(filter)?;
    let text = if *reveal {
        format!("reveal {cards} from outside the game and put it into your hand")
    } else {
        format!("put {cards} from outside the game into your hand")
    };
    Some((text, 2))
}
