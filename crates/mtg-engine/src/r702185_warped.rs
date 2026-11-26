//! Warped cards (CR 702.185): a card in exile that its own warp ability exiled, read by `Pred::R702185Warped` ("a warped
//! creature card you own in exile"). The warp rider exiles the permanent and lets its owner cast the card from exile; the
//! card is warped while that permission, given by the permanent it was, names it.

use crate::object::ObjectRef;
use crate::rules::Rule;
use crate::state::GameState;
use crate::zones::Zone;

/// Whether `card` is a warped card: in exile as that object, and named by a permission to cast it from exile that the
/// permanent it was gave.
pub(crate) fn r702185_warped(state: &GameState, card: ObjectRef) -> bool {
    let in_exile = state
        .object(card.id)
        .is_some_and(|object| object.counter() == card.counter && object.zone() == Zone::Exile);
    in_exile
        && crate::rules::rules_by_source(state)
            .iter()
            .any(|(rule, _, source)| match rule {
                Rule::MayCast {
                    only: Some(only),
                    from: Some(mtg_cards::refs::ZoneKind::Exile),
                    ..
                } => {
                    only.contains(&card)
                        && source.is_some_and(|was| was.id == card.id && was.counter < card.counter)
                }
                _ => false,
            })
}
