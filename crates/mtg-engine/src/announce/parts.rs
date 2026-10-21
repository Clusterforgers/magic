//! The permanents and cards that pay the parts of an ability's cost (CR 601.2b, 601.2h).
//!
//! A part that takes objects (a sacrifice, a discard, an exile from a graveyard, tapped creatures, a return to hand) asks
//! `ChooseCards` for them, one part at a time in the order the cost prints them, from what the player controls or holds
//! and no earlier part has taken. A part with no more candidates than it takes is not asked; one with fewer cannot be
//! paid and rewinds the announcement. The picks are kept in `Choices.cost_picks` and paid with the events of the
//! activation (`cast.rs`).

use mtg_cards::ability::Ability;

use super::{Ctx, Next, What, Work};
use crate::choose::CardsOptions;
use crate::decision::DecisionKind;
use crate::parts::{Part, Payer};
use crate::price::Price;

/// The price of the cost of an activated ability, when it has parts beyond `{T}`, mana and the loss of the source.
pub(super) fn read(what: &What, rules: Option<&Ability>) -> Option<Price> {
    match (what, rules?) {
        (What::Ability(_), Ability::Activated { cost, .. }) => Price::announced(cost)
            .ok()
            .filter(|price| !price.parts.is_empty()),
        _ => None,
    }
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let price = ctx.needs.price.as_ref()?;
    if work.choices.cost_picks().is_empty()
        && price.claimed(ctx.state, ctx.player, ctx.source).is_none()
    {
        return Some(Next::Drop);
    }
    let mut settled = 0;
    for part in &price.parts {
        let Part::Pick(pick) = part else { continue };
        if settled < work.choices.cost_picks().len() {
            settled += 1;
            continue;
        }
        let barred = price.barred(part, ctx.source, work.choices.cost_picks());
        let candidates = pick.candidates(&Payer {
            state: ctx.state,
            player: ctx.player,
            source: ctx.source,
            barred: &barred,
        });
        if candidates.len() < pick.count {
            return Some(Next::Drop);
        }
        // Nothing to choose between: the part takes what there is.
        if candidates.len() == pick.count {
            work.choices.pick_for_cost(candidates);
            settled += 1;
            continue;
        }
        let count = u32::try_from(pick.count).unwrap_or(u32::MAX);
        return Some(ctx.ask(DecisionKind::ChooseCards(CardsOptions {
            source: ctx.source,
            from: candidates,
            min: count,
            max: count,
            text: pick.text(),
            can_cancel: ctx.can_cancel(),
        })));
    }
    None
}
