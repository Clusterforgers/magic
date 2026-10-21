//! The value of X (CR 601.2b, 107.3).
//!
//! A cost with `{X}` asks `ChooseNumber` for X, from 0 to the most the player can pay for with the rest of the total
//! cost; X is kept in `Choices.x`, adds that much generic mana for each `{X}` to the total cost and outlives the
//! spell. A card has no `{X}` for the announcement until (g2) reads it from the cost.

use mtg_core::mana::{ManaCost, ManaSymbol};

use super::{Ctx, Next, Work, total};
use crate::choose::{Choices, NumberOptions};
use crate::decision::DecisionKind;

/// The most the search for the largest X goes on: a payment this large is more mana than any game has.
const MOST_X: u32 = 1_000;

/// An `{X}` in a cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct XCost {
    /// How many `{X}` the cost holds; each costs X generic mana.
    pub(crate) symbols: u32,
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    if work.done.x {
        return None;
    }
    if ctx.needs.x.is_none() {
        work.done.x = true;
        return None;
    }
    // Without a value of X that can be paid for there is nothing to announce.
    let Some(max) = largest(ctx, work.choices) else {
        return Some(Next::Drop);
    };
    Some(ctx.ask(DecisionKind::ChooseNumber(NumberOptions {
        source: ctx.source,
        min: 0,
        max: Some(max),
        text: "Choose a value for X.".to_owned(),
        can_cancel: ctx.can_cancel(),
    })))
}

/// `mana` with X paid: X generic mana for each `{X}` of the cost.
pub(super) fn add(ctx: &Ctx<'_>, choices: &Choices, mana: Option<ManaCost>) -> Option<ManaCost> {
    let Some(cost) = &ctx.needs.x else {
        return mana;
    };
    let generic = choices.x.unwrap_or(0).saturating_mul(cost.symbols);
    Some(
        mana.unwrap_or(ManaCost::ZERO)
            .with(ManaSymbol::Generic(generic)),
    )
}

/// The largest X whose total cost the player can pay, or `None` when not even X = 0 can be paid.
fn largest(ctx: &Ctx<'_>, choices: &Choices) -> Option<u32> {
    let mut trial = choices.clone();
    let mut best = None;
    for x in 0..=MOST_X {
        trial.x = Some(x);
        if !total::of(ctx, &trial).is_some_and(|total| total::affordable(ctx, &total)) {
            break;
        }
        best = Some(x);
    }
    best
}
