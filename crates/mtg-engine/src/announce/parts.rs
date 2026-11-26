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
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Ctx, Needs, Next, What, Work, x};
use crate::choose::{CardsOptions, ChoiceOption, Choices, OptionOptions};
use crate::decision::DecisionKind;
use crate::error::IllegalAnswer;
use crate::parts::{Part, Payer};
use crate::price::Price;
use crate::state::GameState;

/// The price of the cost of an activated ability, when it has parts beyond `{T}`, mana and the loss of the source.
pub(super) fn read(
    state: &GameState,
    player: PlayerId,
    what: &What,
    rules: Option<&Ability>,
) -> Option<Box<Price>> {
    match (what, rules?) {
        (What::Ability(ability), activated @ Ability::Activated { cost, .. }) => {
            Price::announced_in(
                &crate::free_activation::cost(state, player, (*ability, activated), cost),
                state,
                player,
                ability.object,
                ability.source.granter(),
            )
            .ok()
            .filter(|price| !price.parts.is_empty())
            .map(Box::new)
        }
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
    if let Some(next) = super::r110_cast_choice::r110_run(ctx, work) {
        return Some(next);
    }
    super::r6012f_any_amount::r6012f_note(ctx, work);
    if let Some(next) = super::r114_sticker_kick::r114_run(ctx, work) {
        return Some(next);
    }
    let price = price_of(ctx, work.choices)?;
    if work.choices.cost_picks().is_empty()
        && price.claimed(ctx.state, ctx.player, ctx.source).is_none()
    {
        return Some(Next::Drop);
    }
    let mut settled = 0;
    for part in &price.parts {
        // An X the text counts from the targets is known once they are chosen: the parts from this one on are picked
        // then (CR 601.2b, 601.2c, 601.2h).
        if part.is_counted()
            && work.choices.x.is_none()
            && super::r5523_x_cap::r116_x_of_targets(ctx).is_some()
        {
            if !super::targets::r117_all_chosen(ctx, work.choices) {
                break;
            }
            work.choices.x = super::r5523_x_cap::r116_x_with_targets(ctx, work.choices);
        }
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
        let barred = ctx.barred(price.barred(&part, ctx.source, work.choices.cost_picks()));
        let candidates = pick.candidates(&Payer {
            state: ctx.state,
            player: ctx.player,
            source: ctx.source,
            barred: &barred,
            granter: price.granter,
            pays: crate::r4707_pay_costs::PaysFor::Cost,
        });
        if pick.short(ctx.state, &candidates) {
            return Some(Next::Drop);
        }
        // Nothing to choose between: the part takes what there is, or nothing for X = 0.
        if let Some(taken) = pick.unasked(ctx.state, &candidates) {
            work.choices.pick_for_cost(taken);
            settled += 1;
            continue;
        }
        let (min, max) = pick.bounds(candidates.len());
        let groups = pick.r4709_groups(ctx.state, &candidates);
        let targeted = if pick.kind.moves() {
            r6012h_targeted(work.choices, &candidates)
        } else {
            Vec::new()
        };
        return Some(ctx.ask(DecisionKind::ChooseCards(CardsOptions {
            one_per_name: false,
            source: ctx.source,
            from: candidates,
            min,
            max,
            text: pick.text(),
            can_cancel: ctx.can_cancel(),
            total: None,
            groups,
            targeted: Box::new(targeted),
        })));
    }
    if let Some(next) = r109_opponent(ctx, work, &price) {
        return Some(next);
    }
    super::w2f_counter_kinds::w2f_run(ctx, work, &price)
}

/// The `candidates` already chosen as targets (an X counted from the targets asks its parts after them, CR 601.2c).
fn r6012h_targeted(choices: &Choices, candidates: &[ObjectId]) -> Vec<ObjectId> {
    candidates
        .iter()
        .copied()
        .filter(|candidate| {
            choices.targets.iter().flatten().any(|target| {
                target
                    .object()
                    .is_some_and(|object| object.id == *candidate)
            })
        })
        .collect()
}

/// The objects the parts picked so far move when paid (sacrificed, exiled, returned): a target among them is a new
/// object by the time the spell or ability resolves (CR 400.7, 601.2c, 601.2h).
pub(super) fn r6012h_moved(ctx: &Ctx<'_>, choices: &Choices) -> Vec<ObjectId> {
    let Some(price) = price_of(ctx, choices) else {
        return Vec::new();
    };
    price
        .parts
        .iter()
        .filter_map(|part| match part.at(choices.x.unwrap_or(0)) {
            Part::Pick(pick) => Some(pick.kind.moves()),
            _ => None,
        })
        .zip(choices.cost_picks())
        .filter(|(moves, _)| *moves)
        .flat_map(|(_, picked)| picked.iter().copied())
        .collect()
}

/// Settles the opponent of each part that has one gain life ("have an opponent gain 3 life"), or asks the payer for the
/// next one with `ChooseOption`; `None` once each has its opponent. A lone opponent is taken without asking.
fn r109_opponent(ctx: &Ctx<'_>, work: &mut Work<'_>, price: &Price) -> Option<Next> {
    let wanted = price
        .parts
        .iter()
        .filter(|part| matches!(part, Part::OpponentGainsLife(_)))
        .count();
    while work.choices.cost_players().len() < wanted {
        let opponents = crate::parts::r109_opponent_life::r109_opponents(ctx.state, ctx.player);
        let index = match (opponents.len(), work.done.w2f_kind_picked.take()) {
            (0, _) => return Some(Next::Drop),
            (1, _) => 0,
            (_, Some(index)) => usize::from(index),
            (_, None) => {
                return Some(
                    ctx.ask(DecisionKind::ChooseOption(OptionOptions {
                        source: ctx.source,
                        options: opponents
                            .iter()
                            .map(|player| ChoiceOption::Player(*player))
                            .collect(),
                        text: "Choose an opponent to gain life.".to_owned(),
                        can_cancel: ctx.can_cancel(),
                    })),
                );
            }
        };
        work.choices.r109_player_for_cost(*opponents.get(index)?);
    }
    None
}

/// The most X the counted parts of the cost allow, `None` when no part is counted: each part can take as much as the
/// player has of what it takes, after the picks made for the parts before it.
pub(super) fn most(ctx: &Ctx<'_>, choices: &Choices) -> Option<u32> {
    let price = price_of(ctx, choices)?;
    price
        .parts
        .iter()
        .filter(|part| part.is_counted())
        .filter_map(|part| {
            let barred = ctx.barred(price.barred(part, ctx.source, choices.cost_picks()));
            part.most(&Payer {
                state: ctx.state,
                player: ctx.player,
                source: ctx.source,
                barred: &barred,
                granter: price.granter,
                pays: crate::r4707_pay_costs::PaysFor::Cost,
            })
        })
        .min()
}

/// The values of X from `min` to `max` that leave a part picking by X ("a red card with mana value X") too little to
/// take, so they are not offered.
pub(super) fn r301_x_without_picks(
    ctx: &Ctx<'_>,
    choices: &Choices,
    min: u32,
    max: u32,
) -> Vec<u32> {
    let Some(price) = price_of(ctx, choices) else {
        return Vec::new();
    };
    let reading: Vec<&Part> = price
        .parts
        .iter()
        .filter(|part| part.r301_picks_by_x() || part.r6702_totals_x())
        .collect();
    if reading.is_empty() {
        return Vec::new();
    }
    (min..=max)
        .filter(|x| {
            reading.iter().any(|part| {
                let Part::Pick(pick) = part.at(*x) else {
                    return false;
                };
                let barred = ctx.barred(price.barred(part, ctx.source, choices.cost_picks()));
                let candidates = pick.candidates(&Payer {
                    state: ctx.state,
                    player: ctx.player,
                    source: ctx.source,
                    barred: &barred,
                    granter: price.granter,
                    pays: crate::r4707_pay_costs::PaysFor::Cost,
                });
                pick.short(ctx.state, &candidates)
            })
        })
        .collect()
}

/// The power the next pick of the cost needs and the power of `cards`, when the part takes tapped creatures for their
/// total power and `cards` fall short of it.
pub(super) fn weak(
    what: &What,
    needs: &Needs,
    choices: &Choices,
    state: &GameState,
    cards: &[ObjectId],
) -> Option<IllegalAnswer> {
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
