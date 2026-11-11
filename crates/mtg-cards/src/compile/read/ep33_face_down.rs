//! Cards an effect puts onto the battlefield face down (CR 708.3, `Destination::BattlefieldFaceDown`): "return it to the
//! battlefield face down under your control", and "return ~ from your graveyard to the battlefield face up or face down".

use super::effects::dead_card;
use crate::effect::{Destination, Effect};
use crate::refs::Ref;

/// "return it to the battlefield face down under your control" (or "under its owner's control"): the card itself, or its
/// card in the graveyard after it died.
pub(super) fn ep33_put_face_down(effect: &Effect) -> Option<String> {
    let Effect::Move {
        what,
        to: Destination::BattlefieldFaceDown { under },
        bind: None,
    } = effect
    else {
        return None;
    };
    if *what != Ref::This && *what != dead_card() {
        return None;
    }
    let whose = match under {
        None => "its owner's",
        Some(Ref::You) => "your",
        Some(_) => return None,
    };
    Some(format!(
        "return it to the battlefield face down under {whose} control"
    ))
}

/// "return ~ from your graveyard to the battlefield face up or face down": the card's controller picks how it enters.
pub(super) fn ep33_face_up_or_down(first: &[Effect], second: &[Effect]) -> Option<String> {
    let [
        Effect::Move {
            what: up,
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters,
                },
            bind: None,
        },
    ] = first
    else {
        return None;
    };
    let [
        Effect::Move {
            what: down,
            to: Destination::BattlefieldFaceDown { under: None },
            bind: None,
        },
    ] = second
    else {
        return None;
    };
    (*up == Ref::This && *down == Ref::This && counters.is_empty())
        .then(|| "return ~ from your graveyard to the battlefield face up or face down".to_owned())
}
