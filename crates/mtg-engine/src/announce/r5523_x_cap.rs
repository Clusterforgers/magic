//! The most X a spell may announce under "X can't be greater than ..." (`CostItem::XAtMost` in a required
//! `AdditionalCost`, Open the Way), read as the spell is announced.

use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;

use super::{Ctx, What};
use crate::resolve::Scope;

/// The least of the caps the spell's required additional costs set on X, never below 0; `None` without one or for
/// anything but a spell.
pub(super) fn r5523_spell_cap(ctx: &Ctx<'_>) -> Option<u32> {
    let What::Spell(spell) = ctx.what else {
        return None;
    };
    let object = ctx.state.object(*spell)?;
    let scope = Scope::announcing(ctx.state, ctx.player, object.reference());
    crate::ability_reader::printed_rules(object.card())
        .filter_map(|rule| match &**rule {
            Ability::AdditionalCost {
                cost,
                required: true,
                ..
            } => Some(cost),
            _ => None,
        })
        .flat_map(|cost| cost.0.iter())
        .filter_map(|item| match item {
            CostItem::XAtMost(most) => scope.amount(most).ok(),
            _ => None,
        })
        .map(|most| u32::try_from(most.max(0)).unwrap_or(u32::MAX))
        .min()
}
