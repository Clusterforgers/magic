//! A return of a group to the hand of their owner: "Return all attacking creatures to their owner's hand." The
//! renderers write "to their owners' hands".

use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::Ref;

const MANY: &str = " to their owners' hands";
const ONE: &str = " to their owner's hand";

/// Adds the wordings of a mass bounce that name one hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let bounces = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Move {
                what: Ref::Each(_),
                to: Destination::Hand
            }
        )
    });
    if !bounces {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(MANY))
        .map(|text| text.replacen(MANY, ONE, 1))
        .collect();
    texts.extend(said);
}
