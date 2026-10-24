//! A permanent that may exile itself and come back: "you may exile ~. If you do, return it to the battlefield under its
//! owner's control."

use crate::effect::{Destination, Effect};
use crate::refs::Ref;

/// The sentences of a choice to exile the source and return it: the exile and the return after "If you do".
pub(super) fn text(effects: &[Effect]) -> Option<String> {
    let [
        Effect::Exile {
            what: Ref::This,
            bind: Some(exiled),
            duration: None,
        },
        Effect::Move {
            what: Ref::Bound(moved),
            to:
                Destination::Battlefield {
                    tapped: false,
                    under,
                    transformed: false,
                    counters,
                },
        },
    ] = effects
    else {
        return None;
    };
    if exiled != moved || !counters.is_empty() {
        return None;
    }
    let owner = match under {
        None => "its owner's",
        Some(Ref::You) => "your",
        Some(_) => return None,
    };
    Some(format!(
        "you may exile ~. If you do, return it to the battlefield under {owner} control"
    ))
}
