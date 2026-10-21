//! The modes of a modal spell or ability (CR 601.2b, 700.2).
//!
//! A mode is offered only when each of its target specifications has enough legal targets; with fewer legal modes than
//! the fewest to choose, the announcement is rewound.

use super::{Ctx, Next, Work};
use crate::choose::{modal_of, mode_options};
use crate::decision::DecisionKind;

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    if work.done.modes {
        return None;
    }
    if let Some(modal) = ctx.ability.and_then(modal_of) {
        let Some(options) = mode_options(
            ctx.state,
            ctx.source,
            ctx.player,
            &modal,
            ctx.can_cancel(),
            ctx.cause(),
        ) else {
            return Some(Next::Drop);
        };
        // Nothing to choose between when no mode may be chosen.
        if options.max > 0 {
            return Some(ctx.ask(DecisionKind::ChooseMode(options)));
        }
    }
    work.done.modes = true;
    None
}
