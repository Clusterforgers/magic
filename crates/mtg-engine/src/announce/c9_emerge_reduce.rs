//! The generic mana emerge and harmonize take off the total cost (CR 702.119a, 702.180a): the mana value of the creature
//! sacrificed for emerge, the power of the creature tapped for harmonize. Before the creature is picked the one that
//! takes off the most is assumed, so a cast is offered when some creature makes it affordable.

use mtg_core::mana::ManaCost;

use super::{Ctx, What};
use crate::c9_emerge_ways::{c9_emerge_measure, c9_emerge_most, c9_emerge_reduced};
use crate::choose::Choices;

/// How much generic mana the way `choices` announced takes off: 0 for any way but emerge and harmonize with a creature
/// tapped.
fn c9_emerge_less(ctx: &Ctx<'_>, choices: &Choices) -> u32 {
    let Some(measure) = c9_emerge_measure(choices.way, choices.cost_variant()) else {
        return 0;
    };
    let What::Spell(spell) = ctx.what else {
        return 0;
    };
    match choices.cost_picks().first() {
        Some(picked) if !picked.is_empty() => picked
            .iter()
            .map(|object| measure(ctx.state, *object))
            .fold(0, u32::saturating_add),
        _ => c9_emerge_most(
            ctx.state,
            ctx.player,
            *spell,
            &ctx.needs.cost_parts(choices),
            measure,
        ),
    }
}

/// `mana` with the reduction of emerge or harmonize taken off its generic part, down to nothing.
pub(super) fn c9_emerge_reduce(
    ctx: &Ctx<'_>,
    choices: &Choices,
    mana: Option<ManaCost>,
) -> Option<ManaCost> {
    let less = c9_emerge_less(ctx, choices);
    if less == 0 {
        return mana;
    }
    mana.map(|cost| c9_emerge_reduced(&cost, less))
}
