//! The value of X (CR 601.2b, 107.3).
//!
//! A cost with `{X}` asks `ChooseNumber` for X, from 0 to the most the player can pay for with the rest of the total
//! cost; X is kept in `Choices.x`, adds that much generic mana for each `{X}` to the total cost and outlives the
//! announcement: the object on the stack reads it as `Amount::X` while it resolves. The `-X` of a loyalty ability asks
//! the same question, from 0 to the loyalty counters of the planeswalker, and removes that many.

use mtg_cards::ability::{Ability, LoyaltyCost};
use mtg_cards::value::CounterKind;
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::{Ctx, Next, What, Work, total};
use crate::choose::{Choices, NumberOptions};
use crate::decision::DecisionKind;
use crate::price::Price;
use crate::state::GameState;

/// The most the search for the largest X goes on: a payment this large is more mana than any game has.
const MOST_X: u32 = 1_000;

/// The X a spell or ability asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum XCost {
    /// An `{X}` in a mana cost; each costs X generic mana.
    Mana {
        /// How many `{X}` the cost holds.
        symbols: u32,
    },
    /// The `-X` of a loyalty ability: X loyalty counters are removed.
    Loyalty,
}

impl XCost {
    /// The X that `what` asks for, read from the cost printed on the card or in its rules: `rules` are the rules of
    /// the ability being announced.
    pub(crate) fn of(state: &GameState, what: &What, rules: Option<&Ability>) -> Option<XCost> {
        let symbols = match what {
            What::Spell(spell) => u32::from(state.object(*spell)?.card().x_in_cost),
            What::Ability(_) => match rules? {
                Ability::Activated { cost, .. } => Price::announced(cost).ok()?.x,
                Ability::Loyalty {
                    cost: LoyaltyCost::MinusX,
                    ..
                } => return Some(XCost::Loyalty),
                _ => 0,
            },
            What::Trigger(_) => 0,
        };
        (symbols > 0).then_some(XCost::Mana { symbols })
    }
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
    // Nothing to choose between: X is 0.
    if max == 0 {
        work.choices.x = Some(0);
        work.done.x = true;
        return None;
    }
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
    let Some(XCost::Mana { symbols }) = &ctx.needs.x else {
        return mana;
    };
    let generic = choices.x.unwrap_or(0).saturating_mul(*symbols);
    Some(
        mana.unwrap_or(ManaCost::ZERO)
            .with(ManaSymbol::Generic(generic)),
    )
}

/// The largest X the player can announce, or `None` when not even X = 0 can be paid.
fn largest(ctx: &Ctx<'_>, choices: &Choices) -> Option<u32> {
    match ctx.needs.x.as_ref()? {
        XCost::Loyalty => Some(
            ctx.state
                .object(ctx.source)?
                .permanent()?
                .counters_of(&CounterKind::Loyalty),
        ),
        XCost::Mana { .. } => largest_payable(ctx, choices),
    }
}

/// The largest X whose total cost the player can pay. More X is more generic mana, so what can be paid for is a run of
/// values from 0, and a bisection finds where it ends.
fn largest_payable(ctx: &Ctx<'_>, choices: &Choices) -> Option<u32> {
    let mut trial = choices.clone();
    let mut payable = |x: u32| {
        trial.x = Some(x);
        total::of(ctx, &trial).is_some_and(|total| total::affordable(ctx, &total))
    };
    if !payable(0) {
        return None;
    }
    let (mut low, mut high) = (0, MOST_X);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if payable(middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    Some(low)
}
