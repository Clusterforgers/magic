//! A spell begun as though it had flash through a permission whose condition reads the spell ("You may cast this spell as
//! though it had flash if it targets a permanent you control", "... if X is 3 or less"): the condition is judged once
//! the spell's X and targets are announced (CR 601.2e), and a cast it does not allow is rewound.

use mtg_cards::value::Condition;
use mtg_core::card::CardType;

use super::{Ctx, What, Work};
use crate::resolve::Scope;
use crate::rules::cast_permissions;

/// Whether the spell announced may be cast now: it is cast at a time a sorcery could be, it needs no permission for its
/// timing, or a permission lets it be cast as though it had flash, one whose condition is judged on the spell holding
/// with what was announced.
pub(super) fn r401_flash_holds(ctx: &Ctx<'_>, work: &Work<'_>) -> bool {
    let What::Spell(id) = ctx.what else {
        return true;
    };
    if ctx.granted.is_some() || crate::priority::sorcery_timing(ctx.state, ctx.player) {
        return true;
    }
    let Some(card) = ctx.state.object(*id) else {
        return true;
    };
    if card.base_types().contains(CardType::Instant)
        || crate::keywords::grants_instant_timing(ctx.state, *id)
    {
        return true;
    }
    let flashing: Vec<_> = cast_permissions(ctx.state)
        .iter()
        .filter(|cast| cast.flash && cast.covers(ctx.state, ctx.player, card))
        .collect();
    let on_cast = |cast: &&crate::rules::Cast| match &cast.when {
        Some(Condition::R401OnCast(inner)) => Some(inner.as_ref().clone()),
        _ => None,
    };
    if flashing.is_empty() || flashing.iter().any(|cast| on_cast(cast).is_none()) {
        return true;
    }
    flashing.iter().any(|cast| {
        on_cast(cast).is_some_and(|inner| {
            let mut scope = Scope::announcing(ctx.state, cast.controller, cast.source);
            scope.choices = &*work.choices;
            scope.holds(&inner).unwrap_or(false)
        })
    })
}
