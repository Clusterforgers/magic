//! Cost increases and reductions (CR 601.2f, 118.7): what the cost modifiers in play do to the cost, after X and the
//! additional costs are added. Nothing modifies a cost until (g5).

use mtg_core::mana::ManaCost;

use super::Ctx;
use crate::choose::Choices;

/// `mana` after every cost modifier that applies to the spell or ability has changed it.
pub(super) const fn apply(
    _ctx: &Ctx<'_>,
    _choices: &Choices,
    mana: Option<ManaCost>,
) -> Option<ManaCost> {
    mana
}
