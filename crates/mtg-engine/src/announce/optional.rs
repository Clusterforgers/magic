//! The optional costs paid in addition to the cost (CR 601.2b, 118.8): kicker, buyback, entwine, "you may pay".
//!
//! The player chooses which of the costs the rules offer (`ChooseCosts`); their mana joins the total cost. A card
//! offers none until (g3).

use mtg_core::mana::ManaCost;

use super::{Ctx, Next, Work};
use crate::choose::{Choices, CostsOptions, OptionalCost};
use crate::decision::DecisionKind;

/// A cost the player may pay in addition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Optional {
    /// The cost as the card prints it.
    pub(crate) label: String,
    /// The mana it adds to the total cost.
    pub(crate) mana: ManaCost,
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    if work.done.optional {
        return None;
    }
    if ctx.needs.optional.is_empty() {
        work.done.optional = true;
        return None;
    }
    let optional = ctx
        .needs
        .optional
        .iter()
        .enumerate()
        .map(|(index, cost)| OptionalCost {
            index,
            label: cost.label.clone(),
        })
        .collect();
    Some(ctx.ask(DecisionKind::ChooseCosts(CostsOptions {
        source: ctx.source,
        optional,
        can_cancel: ctx.can_cancel(),
    })))
}

/// `mana` with the mana of the optional costs chosen added.
pub(super) fn add(ctx: &Ctx<'_>, choices: &Choices, mana: Option<ManaCost>) -> Option<ManaCost> {
    let paid: Vec<&ManaCost> = choices
        .optional_costs
        .iter()
        .filter_map(|index| ctx.needs.optional.get(*index))
        .map(|cost| &cost.mana)
        .collect();
    if paid.is_empty() {
        return mana;
    }
    let symbols = mana
        .iter()
        .chain(paid)
        .flat_map(|cost| cost.symbols().iter().copied());
    Some(ManaCost::from_symbols(symbols))
}
