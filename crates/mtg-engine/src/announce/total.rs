//! The total cost (CR 601.2f): the cost printed, replaced by an alternative cost, with the optional costs and X added
//! and the cost modifiers applied, in that order.
//!
//! The mana of the total is what the payment step asks for, from the sources that may pay it.

use mtg_cards::ability::Ability;
use mtg_core::mana::ManaCost;

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
    /// The life other parts of the cost pay, which the mana cannot use.
    pub(crate) life: u32,
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
    let (printed, sources, life) = printed(ctx, choices)?;
    let mana = alternative::replace(ctx, choices, printed);
    let mana = optional::add(ctx, choices, mana);
    let mana = x::add(ctx, choices, mana);
    let mana = modifiers::apply(ctx, choices, mana);
    Some(Total {
        mana,
        sources,
        life,
    })
}

/// The cost as the card prints it and the sources that may pay its mana: the mana cost of a spell, the cost of an
/// activated ability (never paid from its own source when that is tapped or lost for it), nothing for a loyalty cost
/// (counters, paid with the events) or a triggered ability.
fn printed(ctx: &Ctx<'_>, choices: &Choices) -> Option<(Option<ManaCost>, Vec<ManaOption>, u32)> {
    match ctx.what {
        What::Spell(spell) => {
            let cost = ctx.state.object(*spell)?.card().mana_cost.clone();
            Some((cost, mana_options(ctx.state, ctx.player), 0))
        }
        What::Ability(ability) => match ctx.ability? {
            Ability::Activated { cost, .. } => {
                let price = Price::announced(cost).ok()?;
                let mut sources = price.sources(ctx.state, ctx.player, ability.object);
                // What the cost sacrifices, taps or returns is not there to tap for mana.
                sources.retain(|option| {
                    !choices
                        .cost_picks()
                        .iter()
                        .flatten()
                        .any(|picked| *picked == option.source.object)
                });
                let life = price.life();
                Some((price.mana, sources, life))
            }
            _ => Some((None, Vec::new(), 0)),
        },
        What::Trigger(_) => Some((None, Vec::new(), 0)),
    }
}

/// Whether the player can pay the mana of `total` with the mana floating and the permanents they control.
pub(super) fn affordable(ctx: &Ctx<'_>, total: &Total) -> bool {
    let Some(seat) = ctx.state.player(ctx.player) else {
        return false;
    };
    let life = seat
        .life()
        .saturating_sub(i32::try_from(total.life).unwrap_or(i32::MAX));
    total
        .mana
        .as_ref()
        .is_none_or(|cost| can_afford(cost, seat.mana(), &total.sources, life))
}
