//! Unattaching a chosen permanent as a cost (CR 701.3d): "Unattach an Equipment from Captain America". The payer picks
//! one attached permanent the filter names; it stays on the battlefield, unattached, and the ability reads it as
//! `Ref::Paid`.

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::{Kind, Part, Payer, Pick};
use crate::zones::Zone;

/// The one pick of the cost.
pub(super) fn lc4_pick(filter: &Selector) -> (Part, bool) {
    (
        Part::Pick(Pick {
            kind: Kind::Lc4Unattach,
            filter: filter.clone(),
            count: 1,
            reads_x: false,
            alike: None,
            up_to: false,
            r6702_total: false,
            r122_crew: false,
        }),
        false,
    )
}

/// The permanents the filter selects that are attached to something, so they can let go.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Battlefield).iter().copied(),
        filter,
        |id| {
            payer
                .state
                .object(id)
                .and_then(|object| object.permanent())
                .is_some_and(|permanent| {
                    permanent.attached_to().is_some() || permanent.attached_player().is_some()
                })
        },
    )
}

/// "Unattach an Equipment attached to this permanent".
pub(super) fn text(filter: &Selector) -> String {
    format!("Unattach {}", crate::wording::subject(filter))
}
