//! Replacements of a keyword action (explore, connive, proliferate): which steps of an `Instead` are the replaced
//! action itself and which run around it.

use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// Whether `when` is the replacement of a keyword action.
#[must_use]
pub const fn r406_is_action(when: &ReplaceWhen) -> bool {
    matches!(
        when,
        ReplaceWhen::Explores { .. }
            | ReplaceWhen::Connives { .. }
            | ReplaceWhen::Proliferates { .. }
    )
}

/// Whether `effect`, a step of an `Instead` of `when`, is the replaced action itself: the exploration or connive of
/// `Triggering`, or the proliferation of the player who would proliferate.
#[must_use]
pub fn r406_is_replaced(when: &ReplaceWhen, effect: &Effect) -> bool {
    match (when, effect) {
        (ReplaceWhen::Explores { .. }, Effect::Explore { what }) => *what == Ref::Triggering,
        (ReplaceWhen::Connives { .. }, Effect::Connive { what, count }) => {
            *what == Ref::Triggering && count.is_one()
        }
        (ReplaceWhen::Proliferates { who }, Effect::Proliferate { who: by }) => {
            *by == Ref::Triggering || (*who == Ref::You && *by == Ref::You)
        }
        _ => false,
    }
}

/// Whether `effect` may run around the replaced action: a step of one event that needs no other step.
const fn r406_around(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::Scry { .. } | Effect::Draw { .. } | Effect::Surveil { .. }
    )
}

/// The problems of a replacement of a keyword action: only `Instead`, holding the replaced action at least once and
/// otherwise only steps the engine runs around it.
pub fn r406_check(when: &ReplaceWhen, by: &ReplaceBy, mut report: impl FnMut(&'static str)) {
    if !r406_is_action(when) {
        return;
    }
    let ReplaceBy::Instead(effects) = by else {
        report("a keyword action is replaced only by steps that happen instead");
        return;
    };
    if !effects.iter().any(|effect| r406_is_replaced(when, effect)) {
        report("a keyword action replaced instead holds the action of `Triggering` at least once");
    }
    if effects
        .iter()
        .any(|effect| !r406_is_replaced(when, effect) && !r406_around(effect))
    {
        report("only a scry, a draw or a surveil runs around a replaced keyword action");
    }
}
