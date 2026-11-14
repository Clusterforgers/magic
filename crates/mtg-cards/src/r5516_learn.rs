//! The replacement of a learn ("As long as this card is in your graveyard, if you would learn, you may instead return
//! this card to the battlefield."): the shape the engine runs.

use crate::effect::{Destination, Effect};
use crate::refs::Ref;
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The steps that return the source to the battlefield.
#[must_use]
pub fn r5516_return_this() -> Vec<Effect> {
    vec![Effect::Move {
        what: Ref::This,
        to: Destination::Battlefield {
            tapped: false,
            under: None,
            transformed: false,
            counters: Vec::new(),
        },
        bind: None,
    }]
}

/// The problems of a replacement of a learn: a learn is replaced only by returning the source to the battlefield, which
/// its controller may decline, and only a learn takes steps its controller may decline.
pub fn r5516_check(when: &ReplaceWhen, by: &ReplaceBy, mut report: impl FnMut(&'static str)) {
    match (when, by) {
        (ReplaceWhen::Learns { .. }, ReplaceBy::MayInstead(effects))
            if *effects == r5516_return_this() => {}
        (ReplaceWhen::Learns { .. }, _) => {
            report(
                "a learn is replaced only by returning this card to the battlefield, which its controller may decline",
            );
        }
        (_, ReplaceBy::MayInstead(_)) => {
            report("only a learn is replaced by steps its controller may decline");
        }
        _ => {}
    }
}
