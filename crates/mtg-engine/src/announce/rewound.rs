//! The detail of an announcement taken back (`AnnouncementRewound.why`, `docs/engine.md`, Refusals), read off the step
//! that could not be met.

use super::{Ctx, What, Work, total, x};
use crate::choose::Choices;
use crate::decision::{Unplayable, Why};

/// Notes why the announcement is taken back: the reason a step set, else `hindrance`, the reason of the step that could
/// not be met, with its detail.
pub(super) fn note(ctx: &Ctx<'_>, work: &mut Work<'_>, hindrance: Unplayable) {
    let reason = *work.done.rewound.get_or_insert(hindrance);
    if work.done.rewound_why.is_none() {
        work.done.rewound_why = why(ctx, work, reason);
    }
}

/// The detail of `reason`: the total mana asked against the mana at hand, or a cost besides mana. A target is named once
/// the announcement is rewound (`Engine::drop_pending`).
fn why(ctx: &Ctx<'_>, work: &Work<'_>, reason: Unplayable) -> Option<Why> {
    match reason {
        Unplayable::Mana => mana(ctx, work.choices, work.done.alternative),
        Unplayable::Cost => Some(Why::OtherCost),
        Unplayable::Timing
        | Unplayable::LandPlayed
        | Unplayable::Targets
        | Unplayable::Restricted => None,
    }
}

/// The total cost as announced, with the least X when X is not chosen yet, against the mana at hand; before a way of
/// casting is settled, the first way whose total cannot be paid. `None` when the mana could be paid, so something else
/// took the announcement back.
fn mana(ctx: &Ctx<'_>, choices: &Choices, way_settled: bool) -> Option<Why> {
    let ways: Vec<Choices> = if way_settled || ctx.needs.ways.is_empty() {
        vec![choices.clone()]
    } else {
        ctx.needs
            .ways
            .iter()
            .map(|way| {
                let mut trial = choices.clone();
                trial.way = way.alternative;
                trial.choose_cost(way.variant);
                trial
            })
            .collect()
    };
    ways.into_iter().find_map(|trial| short(ctx, trial))
}

/// [`mana`] for one set of choices.
fn short(ctx: &Ctx<'_>, mut choices: Choices) -> Option<Why> {
    if choices.x.is_none() {
        choices.x = Some(x::least_x(ctx, &choices));
    }
    let total = total::of(ctx, &choices)?;
    if total::affordable(ctx, &total) {
        return None;
    }
    let cost = total.mana.clone()?;
    let pool = ctx.state.player(ctx.player)?.mana();
    // The commander tax is in the total unless it is paid in life (CR 903.8).
    let tax = match ctx.what {
        What::Spell(spell)
            if crate::r5500_tax_life::r5500_tax_life(ctx.state, *spell).is_none() =>
        {
            crate::commanders::tax_for(ctx.state, *spell)
        }
        _ => 0,
    };
    Some(crate::why::mana(cost, pool, &total.sources, tax))
}
