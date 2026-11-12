//! The mana cost of the permanent sacrificed for offering comes off the cost of the spell (CR 702.48a). Before the
//! permanent is picked the one that leaves the least to pay is assumed, so a cast is offered when some permanent makes it
//! affordable.

use mtg_cards::value::CastWay;
use mtg_core::mana::ManaCost;

use super::{Ctx, What};
use crate::choose::Choices;
use crate::r301c_offering::{r301c_offering_candidates, r301c_offering_difference};

/// `mana` less the mana cost of the permanent sacrificed for offering; `mana` as it is for any other way.
pub(super) fn r301c_offering_reduce(
    ctx: &Ctx<'_>,
    choices: &Choices,
    mana: Option<ManaCost>,
) -> Option<ManaCost> {
    if choices.way != Some(CastWay::Offering) {
        return mana;
    }
    let What::Spell(spell) = ctx.what else {
        return mana;
    };
    let cost = mana?;
    let state = ctx.state;
    let cost_of = |id| {
        state
            .object(id)
            .and_then(|object| object.mana_cost().cloned())
            .unwrap_or(ManaCost::ZERO)
    };
    Some(match choices.cost_picks().first() {
        Some(picked) if !picked.is_empty() => picked.iter().fold(cost, |left, id| {
            r301c_offering_difference(&left, &cost_of(*id))
        }),
        _ => r301c_offering_candidates(state, ctx.player, *spell, &ctx.needs.cost_parts(choices))
            .into_iter()
            .map(|id| r301c_offering_difference(&cost, &cost_of(id)))
            .min_by_key(ManaCost::mana_value)
            .unwrap_or(cost),
    })
}
