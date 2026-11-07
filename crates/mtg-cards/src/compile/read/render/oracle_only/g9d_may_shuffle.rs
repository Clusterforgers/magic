//! The top cards looked at, put back, and a shuffle the controller may have: "Look at the top three cards of target
//! player's library, then put them back in any order. You may have that player shuffle." and the same of your own library
//! ("You may shuffle."), with a draw after it.

use super::super::super::library::top_text;
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::{Destination, Effect, Order, Place};
use crate::refs::{Ref, Targetable};
use crate::value::Amount;

/// Adds the wordings of a look at the top cards of a library, their return on top in an order the controller chooses,
/// and the shuffle of that library the controller may have, alone or followed by "Draw a card."
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let (Ability::Spell { targets, effects }
    | Ability::Activated {
        targets, effects, ..
    }
    | Ability::Triggered {
        targets, effects, ..
    }) = ability
    else {
        return;
    };
    let [
        Effect::LookAtTop { who, count, bind },
        Effect::Move {
            what: Ref::Bound(back),
            to:
                Destination::Library {
                    place: Place::Top,
                    order: Some(Order::Chosen),
                },
            bind: None,
        },
        Effect::May {
            who: Ref::You,
            effects: optional,
        },
        rest @ ..,
    ] = effects.as_slice()
    else {
        return;
    };
    let [Effect::Shuffle { who: shuffler }] = optional.as_slice() else {
        return;
    };
    if back != bind || shuffler != who {
        return;
    }
    let (library, shuffle) = match (who, targets.as_slice()) {
        (Ref::You, []) => ("your library", "shuffle"),
        (Ref::Target(0), [spec]) if spec.what == Targetable::Player && spec.bounds.is_one() => {
            ("target player's library", "have that player shuffle")
        }
        _ => return,
    };
    let Some(cards) = top_text(count) else {
        return;
    };
    let said =
        format!("look at {cards} of {library}, then put them back in any order. You may {shuffle}");
    let after: &[&str] = match rest {
        [] => &[""],
        [
            Effect::Draw {
                who: Ref::You,
                count,
                bind: None,
            },
        ] if *count == Amount::one() => &[".\nDraw a card", ". Draw a card"],
        _ => return,
    };
    for tail in after {
        texts.extend(with_effects(ability, &format!("{said}{tail}")));
    }
}
