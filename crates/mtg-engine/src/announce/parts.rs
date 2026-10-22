//! The permanents and cards that pay the parts of an ability's cost, and of the optional and alternative costs of a
//! spell (CR 601.2b, 601.2h).
//!
//! A part that takes objects (a sacrifice, a discard, an exile from a graveyard, tapped creatures, a return to hand) asks
//! `ChooseCards` for them, one part at a time in the order the cost prints them, from what the player controls or holds
//! and no earlier part has taken. A part with no more candidates than it takes is not asked; one with fewer cannot be
//! paid and rewinds the announcement. The picks are kept in `Choices.cost_picks` and paid with the events of the
//! activation or the cast (`cast.rs`). The card being cast is never offered for its own cost.

use std::borrow::Cow;

use mtg_cards::ability::Ability;
use mtg_core::ids::ObjectId;

use super::{Ctx, Needs, Next, What, Work, x};
use crate::choose::{CardsOptions, Choices};
use crate::decision::DecisionKind;
use crate::parts::{Part, Payer};
use crate::price::Price;
use crate::state::GameState;

/// The price of the cost of an activated ability, when it has parts beyond `{T}`, mana and the loss of the source.
pub(super) fn read(what: &What, rules: Option<&Ability>) -> Option<Box<Price>> {
    match (what, rules?) {
        (What::Ability(_), Ability::Activated { cost, .. }) => Price::announced(cost)
            .ok()
            .filter(|price| !price.parts.is_empty())
            .map(Box::new),
        _ => None,
    }
}

/// The price whose parts are announced: that of the activated ability, or the parts a spell pays besides its mana
/// (those of the way it is cast and of its optional costs), once those are chosen.
fn price_of<'a>(ctx: &'a Ctx<'_>, choices: &Choices) -> Option<Cow<'a, Price>> {
    match ctx.what {
        What::Spell(_) => {
            let parts = ctx.needs.cost_parts(choices);
            (!parts.is_empty()).then(|| Cow::Owned(Price::casting(parts)))
        }
        _ => ctx.needs.price.as_deref().map(Cow::Borrowed),
    }
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    let price = price_of(ctx, work.choices)?;
    if work.choices.cost_picks().is_empty()
        && price.claimed(ctx.state, ctx.player, ctx.source).is_none()
    {
        return Some(Next::Drop);
    }
    let mut settled = 0;
    for part in &price.parts {
        // A counted part needs X first, which is asked when the parts come to it (CR 601.2b).
        if part.is_counted()
            && let Some(next) = x::run(ctx, work)
        {
            return Some(next);
        }
        let part = part.at(work.choices.x.unwrap_or(0));
        let Part::Pick(pick) = &part else { continue };
        if settled < work.choices.cost_picks().len() {
            settled += 1;
            continue;
        }
        let barred = price.barred(&part, ctx.source, work.choices.cost_picks());
        let candidates = pick.candidates(&Payer {
            state: ctx.state,
            player: ctx.player,
            source: ctx.source,
            barred: &barred,
        });
        if pick.short(ctx.state, &candidates) {
            return Some(Next::Drop);
        }
        // Nothing to choose between: the part takes what there is, or nothing for X = 0.
        if let Some(taken) = pick.unasked(&candidates) {
            work.choices.pick_for_cost(taken);
            settled += 1;
            continue;
        }
        let (min, max) = pick.bounds(candidates.len());
        return Some(ctx.ask(DecisionKind::ChooseCards(CardsOptions {
            source: ctx.source,
            from: candidates,
            min,
            max,
            text: pick.text(),
            can_cancel: ctx.can_cancel(),
        })));
    }
    None
}

/// The most X the counted parts of the cost allow, `None` when no part is counted: each part can take as much as the
/// player has of what it takes, after the picks made for the parts before it.
pub(super) fn most(ctx: &Ctx<'_>, choices: &Choices) -> Option<u32> {
    let price = ctx.needs.price.as_ref()?;
    price
        .parts
        .iter()
        .filter(|part| part.is_counted())
        .filter_map(|part| {
            let barred = price.barred(part, ctx.source, choices.cost_picks());
            part.most(&Payer {
                state: ctx.state,
                player: ctx.player,
                source: ctx.source,
                barred: &barred,
            })
        })
        .min()
}

/// The power the next pick of the cost needs and the power of `cards`, when the part takes tapped creatures for their
/// total power and `cards` fall short of it.
pub(super) fn weak(
    what: &What,
    needs: &Needs,
    choices: &Choices,
    state: &GameState,
    cards: &[ObjectId],
) -> Option<(u32, u32)> {
    let parts = match what {
        What::Spell(_) => needs.cost_parts(choices),
        _ => needs.price.as_deref()?.parts.clone(),
    };
    parts
        .iter()
        .map(|part| part.at(choices.x.unwrap_or(0)))
        .filter_map(|part| match part {
            Part::Pick(pick) => Some(pick),
            _ => None,
        })
        .nth(choices.cost_picks().len())?
        .weak(state, cards)
}
