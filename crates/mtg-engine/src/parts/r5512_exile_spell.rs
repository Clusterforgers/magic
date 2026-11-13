//! Exiling a spell the payer controls as a cost ("Exile an instant or sorcery spell you control"): the spell leaves the
//! stack and never resolves.

use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::object::ObjectKind;
use crate::zones::Zone;

/// The spells on the stack the payer controls that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    let mut on_stack = filter.clone();
    on_stack.0.push(Pred::In(ZoneKind::Stack));
    payer.matching(
        payer.state.zone(Zone::Stack).iter().copied(),
        &on_stack,
        |id| {
            payer.state.object(id).is_some_and(|object| {
                object.kind() != ObjectKind::Ability && object.controller() == payer.player
            })
        },
    )
}

/// "Exile an instant or sorcery spell you control."
pub(super) fn text(filter: &Selector) -> String {
    format!(
        "Exile {}.",
        crate::wording::counted(filter, 1, "you control").replacen(
            " you control",
            " spell you control",
            1
        )
    )
}
