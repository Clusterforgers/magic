//! The card and the graveyard shuffled into the library: "Shuffle this artifact and your graveyard into their owner's
//! library."

use super::super::{Destination, Effect, Place, Pred, Ref, ZoneKind};

/// Whether the destination is the library, shuffled in.
fn shuffled(to: &Destination) -> bool {
    matches!(
        to,
        Destination::Library {
            place: Place::Shuffled,
            order: None
        }
    )
}

/// The sentence for the shuffle of the card itself and of the controller's graveyard into their libraries, and the two
/// effects it covers.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Move {
            what: Ref::This,
            to: first,
        },
        Effect::Move {
            what: Ref::Each(group),
            to: second,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let graveyard = [Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)];
    (shuffled(first) && shuffled(second) && group.0 == graveyard).then(|| {
        (
            "shuffle ~ and your graveyard into their owner's library".to_owned(),
            2,
        )
    })
}
