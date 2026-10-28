//! The generic mana emerge and harmonize take off the total cost (CR 702.119a, 702.180a): the mana value of the creature
//! sacrificed for emerge, the power of the creature tapped for harmonize. Before the creature is picked the one that
//! takes off the most is assumed, so a cast is offered when some creature makes it affordable.

use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::{Ctx, What, modifiers};
use crate::c9_emerge_ways::C9_EMERGE_TAPPED;
use crate::characteristics::characteristics;
use crate::choose::Choices;
use crate::parts::{Part, Payer};
use crate::state::GameState;

/// The mana value of a permanent as it stands.
fn c9_emerge_mana_value(state: &GameState, object: ObjectId) -> u32 {
    state.object(object).map_or(0, crate::object::GameObject::mana_value)
}

/// The power of a permanent as it stands, 0 for none or a negative one.
fn c9_emerge_power(state: &GameState, object: ObjectId) -> u32 {
    characteristics(state, object)
        .and_then(|found| found.power)
        .map_or(0, |power| u32::try_from(power).unwrap_or(0))
}

/// How much generic mana the way `choices` announced takes off: 0 for any way but emerge and harmonize with a creature
/// tapped.
pub(super) fn c9_emerge_less(ctx: &Ctx<'_>, choices: &Choices) -> u32 {
    let measure: fn(&GameState, ObjectId) -> u32 = match choices.way {
        Some(CastWay::Emerge) => c9_emerge_mana_value,
        Some(CastWay::Harmonize) if choices.cost_variant() & C9_EMERGE_TAPPED != 0 => {
            c9_emerge_power
        }
        _ => return 0,
    };
    let What::Spell(spell) = ctx.what else {
        return 0;
    };
    if let Some(picked) = choices.cost_picks().first().filter(|picked| !picked.is_empty()) {
        return picked
            .iter()
            .map(|object| measure(ctx.state, *object))
            .fold(0, u32::saturating_add);
    }
    let Some(Part::Pick(pick)) = ctx.needs.cost_parts(choices).into_iter().next() else {
        return 0;
    };
    let payer = Payer {
        state: ctx.state,
        player: ctx.player,
        source: *spell,
        barred: &[],
    };
    pick.candidates(&payer)
        .into_iter()
        .map(|object| measure(ctx.state, object))
        .max()
        .unwrap_or(0)
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
    mana.map(|cost| modifiers::reduced(&cost, &ManaCost::ZERO.with(ManaSymbol::Generic(less))))
}
