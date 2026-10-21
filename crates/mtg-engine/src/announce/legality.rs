//! Whether the proposed spell or ability can be put on the stack (CR 601.2e).
//!
//! Everything announced so far is checked as a whole: each specification has its targets and each of them is still
//! legal for it, and every amount to divide is shared out. What fails rewinds the announcement.

use super::{Ctx, Next, Work, targets};
use crate::choose::legal_for;

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let specs = targets::specs(ctx, &work.choices.modes);
    let chosen = &work.choices.targets;
    let legal = chosen.len() == specs.len()
        && specs.iter().zip(chosen).all(|(spec, picked)| {
            let legal = legal_for(ctx.state, spec, ctx.source, ctx.player, ctx.cause());
            picked.iter().all(|target| legal.contains(target))
        });
    let shared = specs
        .iter()
        .zip(chosen)
        .filter(|(spec, _)| spec.divide.is_some())
        .all(|(_, picked)| {
            picked.iter().all(|target| {
                work.choices
                    .division
                    .iter()
                    .any(|(named, _)| named == target)
            })
        });
    (!(legal && shared)).then_some(Next::Drop)
}
