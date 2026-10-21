//! The total cost (CR 601.2f): the cost printed, replaced by an alternative cost, with the optional costs and X added
//! and the cost modifiers applied, in that order.
//!
//! The mana of the total is what the payment step asks for, from the sources that may pay it.

use mtg_cards::ability::Ability;
use mtg_core::mana::ManaCost;

use super::payment::spend;
use super::{Ctx, Next, What, Work, alternative, modifiers, optional, x};
use crate::choose::Choices;
use crate::mana::{ManaOption, can_afford};
use crate::price::Price;
use crate::priority::mana_options;

/// What a spell or ability costs to put on the stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Total {
    /// The mana to pay; `None` for a cost without any.
    pub(crate) mana: Option<ManaCost>,
    /// The mana abilities that may pay it.
    pub(crate) sources: Vec<ManaOption>,
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    match of(ctx, work.choices) {
        Some(total) => {
            work.total = Some(total);
            None
        }
        None => Some(Next::Drop),
    }
}

/// The total cost of the spell or ability with `choices` announced, or `None` when it cannot be worked out: the object
/// is gone or its cost has a part the engine does not pay.
pub(super) fn of(ctx: &Ctx<'_>, choices: &Choices) -> Option<Total> {
    let (printed, sources) = printed(ctx)?;
    let mana = alternative::replace(ctx, choices, printed);
    let mana = optional::add(ctx, choices, mana);
    let mana = x::add(ctx, choices, mana);
    let mana = modifiers::apply(ctx, choices, mana);
    Some(Total { mana, sources })
}

/// The cost as the card prints it and the sources that may pay its mana: the mana cost of a spell, the cost of an
/// activated ability (never paid from its own source when that is tapped or lost for it), nothing for a loyalty cost
/// (counters, paid with the events) or a triggered ability.
fn printed(ctx: &Ctx<'_>) -> Option<(Option<ManaCost>, Vec<ManaOption>)> {
    match ctx.what {
        What::Spell(spell) => {
            let cost = ctx.state.object(*spell)?.card().mana_cost.clone();
            Some((cost, mana_options(ctx.state, ctx.player)))
        }
        What::Ability(ability) => match ctx.ability? {
            Ability::Activated { cost, .. } => {
                let price = Price::announced(cost).ok()?;
                let sources = price.sources(ctx.state, ctx.player, ability.object);
                Some((price.mana, sources))
            }
            _ => Some((None, Vec::new())),
        },
        What::Trigger(_) => Some((None, Vec::new())),
    }
}

/// Whether the player can pay the mana of `total` with the mana floating and the permanents they control.
pub(super) fn affordable(ctx: &Ctx<'_>, total: &Total) -> bool {
    let Some(seat) = ctx.state.player(ctx.player) else {
        return false;
    };
    total
        .mana
        .as_ref()
        .is_none_or(|cost| {
            can_afford(cost, seat.mana(), &total.sources, seat.life(), &spend(ctx))
        })
}
