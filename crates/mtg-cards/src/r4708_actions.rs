//! Replacements of a scry or a surveil ("If you would scry a number of cards, draw that many cards instead.", "You may
//! look at an additional two cards each time you surveil."): the shapes the engine runs.

use crate::effect::Effect;
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// Whether `when` is the replacement of a scry or a surveil.
#[must_use]
pub const fn r4708_is_action(when: &ReplaceWhen) -> bool {
    matches!(
        when,
        ReplaceWhen::Scries { .. } | ReplaceWhen::Surveils { .. }
    )
}

/// The problems of a replacement of a scry or a surveil: a scry is replaced only by drawing that many cards instead
/// (the number of cards as `Triggering`) or by scrying a fixed number more ("scry that many cards plus one instead"); a
/// surveil only by more cards the controller may look at, and only those
/// two take more cards.
pub fn r4708_check(when: &ReplaceWhen, by: &ReplaceBy, mut report: impl FnMut(&'static str)) {
    match (when, by) {
        (ReplaceWhen::Scries { .. }, ReplaceBy::Instead(effects))
            if r4708_draws_that_many(effects) => {}
        (ReplaceWhen::Scries { .. }, ReplaceBy::Add(n)) if *n > 0 => {}
        (ReplaceWhen::Scries { .. }, _) => {
            report(
                "a scry is replaced only by drawing that many cards instead or by scrying more cards",
            );
        }
        (ReplaceWhen::Surveils { .. }, ReplaceBy::MayAddAmount(Amount::Fixed(n))) if *n > 0 => {}
        (ReplaceWhen::Surveils { .. }, _) => {
            report(
                "a surveil is replaced only by a fixed number of more cards the controller may look at",
            );
        }
        (_, ReplaceBy::MayAddAmount(_)) => {
            report("only a surveil takes more cards its controller may look at");
        }
        _ => {}
    }
}

/// Whether `effects`, the steps of an `Instead` of a scry, draw the number of cards that would have been scried.
#[must_use]
pub fn r4708_draws_that_many(effects: &[Effect]) -> bool {
    matches!(
        effects,
        [Effect::Draw {
            who: crate::refs::Ref::You,
            count: Amount::Triggering,
            bind: None
        }]
    )
}
