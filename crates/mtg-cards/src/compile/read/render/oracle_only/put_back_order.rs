//! The top cards of the library looked at and put back: "look at the top three cards of your library, then put them back
//! in any order." The renderers word the look alone.

use super::super::variants;
use crate::ability::Ability;
use crate::effect::{Destination, Effect, Order, Place};
use crate::refs::Ref;

/// Adds the wordings of a look at the top cards and the move of them back to the top in an order the player chooses.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [
        look @ Effect::LookAtTop {
            who: Ref::You,
            bind,
            ..
        },
        Effect::Move {
            what: Ref::Bound(back),
            to:
                Destination::Library {
                    place: Place::Top,
                    order: Some(Order::Chosen),
                },
        },
    ] = ability.effects()
    else {
        return;
    };
    if bind != back {
        return;
    }
    let mut plain = ability.clone();
    match &mut plain {
        Ability::Triggered { effects, .. }
        | Ability::Spell { effects, .. }
        | Ability::Activated { effects, .. } => *effects = vec![look.clone()],
        _ => return,
    }
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| format!("{text}, then put them back in any order."))
        .collect();
    texts.extend(said);
}
