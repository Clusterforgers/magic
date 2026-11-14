//! The selector atoms of card pack r55-20: a foretold card (CR 702.143) and an object that has a name (CR 708.2a).

use mtg_cards::refs::Pred;
use mtg_core::ids::ObjectId;

use super::{Scope, Seen};
use crate::choose::Target;
use crate::object::{FaceDown, GameObject, ObjectRef};
use crate::snapshot::Snapshot;
use crate::state::GameState;
use crate::zones::Zone;

/// The name a payment in an effect binds to the objects its parts took ("you may pay {1}{R} and discard a card. If you
/// do, ... the discarded card's mana value"), which `Ref::Paid` reads when the ability's own cost took nothing.
pub(crate) const R5520_PAID: &str = "~paid";

/// The binding of the objects a payment in an effect took, as they were when it took them, with their snapshots: a card
/// it moved is read as it last was (CR 608.2h). Nothing when it took nothing.
pub(crate) fn r5520_took(
    state: &GameState,
    took: &[ObjectId],
) -> (Vec<(String, Vec<Target>)>, Vec<Snapshot>) {
    if took.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let named = took
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|object| Target::Object(GameObject::reference(object)))
        .collect();
    let snapshots = took
        .iter()
        .filter_map(|id| crate::snapshot::snapshot(state, *id))
        .collect();
    (vec![(R5520_PAID.to_owned(), named)], snapshots)
}

impl Scope<'_> {
    /// Whether `Foretold` or `HasName` holds for the object; false for any other predicate.
    pub(super) fn r5520_pred(&self, pred: &Pred, reference: ObjectRef, seen: &Seen<'_>) -> bool {
        // How it lies: the live object while it is the one named, else as its snapshot holds it (CR 608.2h).
        let face_down = match self.state.object(reference.id) {
            Some(object) if object.reference() == reference => object.face_down(),
            _ => self.snapshot_of(reference).and_then(|shot| shot.face_down),
        };
        match pred {
            Pred::Foretold => {
                seen.zone == Zone::Exile && matches!(face_down, Some(FaceDown::Foretold { .. }))
            }
            Pred::HasName => {
                face_down.is_none()
                    && seen.card.is_some_and(|card| {
                        !crate::un34_stickers::name_with(&seen.chars.name_words, &card.card().name)
                            .trim()
                            .is_empty()
                    })
            }
            _ => false,
        }
    }
}
