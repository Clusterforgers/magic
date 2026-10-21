//! The alternative cost a spell is cast for (CR 601.2b, 118.9): flashback, evoke, overload, madness, casting without
//! paying its mana cost.
//!
//! Each way to cast is chosen with the cast itself, as a separate entry of the castable spells, so the announcement
//! asks nothing here; it only reads the way chosen when it works out the total cost. Nothing sets one until (g4).

use mtg_core::mana::ManaCost;

use super::{Ctx, Next, Work};

/// A cost paid instead of the mana cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Alternative {
    /// The mana to pay; the zero cost for a spell cast without paying its mana cost.
    pub(crate) mana: ManaCost,
}

pub(super) const fn run(_ctx: &Ctx<'_>, _work: &mut Work<'_>) -> Option<Next> {
    None
}

/// The mana cost the alternative cost puts in place of `mana`, if the spell is cast for one.
pub(super) fn replace(ctx: &Ctx<'_>, mana: Option<ManaCost>) -> Option<ManaCost> {
    match &ctx.needs.alternative {
        Some(alternative) => Some(alternative.mana.clone()),
        None => mana,
    }
}
