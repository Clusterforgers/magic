//! A commander taken from the command zone: "Put your commander into your hand from the command zone."

use super::super::{Destination, Effect, Pred, Ref, ZoneKind};

/// The sentence for a move of the commander you own from the command zone to the hand, and the one effect it covers.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Move {
            what: Ref::Each(selector),
            to: Destination::Hand,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let commander = [
        Pred::Commander,
        Pred::OwnedBy(Ref::You),
        Pred::In(ZoneKind::Command),
    ];
    (selector.0 == commander).then(|| {
        (
            "put your commander into your hand from the command zone".to_owned(),
            1,
        )
    })
}
