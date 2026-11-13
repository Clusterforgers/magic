//! The `{X}` of a payment an effect asks for ("you may pay {X}{R}. If you do, it deals X damage"): a player who says yes
//! announces X (`ChooseNumber`, from 0 to the most they can pay), pays the cost with X generic mana for each `{X}`, and
//! the effects after the payment read that X (CR 107.3, 107.1c).

use std::iter::Peekable;

use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;

use super::pay::can_pay;
use crate::choose::{Choice, NumberOptions, Target};
use crate::decision::{Decision, DecisionKind};
use crate::price::Price;
use crate::resolve::{Cx, ResolveError, W6C_PAID_X};

/// The most X a search tries: a payment this large is more mana than any game has.
const W6C_MOST_X: u32 = 1_000;

/// Where a player who said yes to a payment stands with its X.
pub(super) enum Announced {
    /// The question for X.
    Ask(Decision),
    /// The price to pay, with the X announced for a cost that has one.
    Price(Price, Option<u32>),
}

/// The price with X announced as `x`: each `{X}` costs X generic mana.
pub(super) fn at(template: &Price, x: u32) -> Price {
    let mut price = template.at(x);
    if price.x > 0 {
        let mana = price.mana.take().unwrap_or(ManaCost::ZERO);
        price.mana = Some(crate::price::with_x(
            mana,
            x.saturating_mul(price.x),
            price.x_color,
        ));
        price.x = 0;
    }
    price
}

/// "Pay {X}{R}": the mana of a payment with its `{X}` symbols in front.
pub(super) fn mana_text(mana: &ManaCost, x_symbols: u32) -> String {
    let xs = "{X}".repeat(usize::try_from(x_symbols).unwrap_or(0));
    if xs.is_empty() || !mana.symbols().is_empty() {
        format!("Pay {xs}{mana}")
    } else {
        format!("Pay {xs}")
    }
}

/// The X of a player who said yes, read from the journal, or the question for it; a cost without `{X}` is paid as it
/// is. X runs from 0 to the most the player can pay, and counts the energy of a part that takes X (`Pay X {E}`) as well as the mana.
pub(super) fn announced<'a>(
    cx: &Cx<'_>,
    player: PlayerId,
    template: &Price,
    (floor, cap): (u32, Option<u32>),
    journal: &mut Peekable<impl Iterator<Item = &'a (PlayerId, Choice)>>,
) -> Result<Announced, ResolveError> {
    // The question offers X up to the most the player can pay, held to the cap of the cost.
    let allowed = |cx: &Cx<'_>| {
        let can = most(cx, player, template);
        cap.map_or(can, |cap| can.min(cap))
    };
    if template.x == 0 && !template.counted() {
        return Ok(Announced::Price(template.clone(), None));
    }
    // The answer was checked against the most the player could pay when it was asked; the mana is spent since.
    match journal.next_if(|entry| entry.0 == player) {
        // A counted part is paid after the answer, so the answer is held to what the player holds now.
        Some((_, Choice::Number(x))) if template.x == 0 && *x > most(cx, player, template) => Err(
            ResolveError::Invalid(format!("{x} is more than {player:?} can pay")),
        ),
        Some((_, Choice::Number(x))) if cap.is_some_and(|cap| *x > cap) => Err(
            ResolveError::Invalid(format!("{x} is more than the X the cost allows")),
        ),
        Some((_, Choice::Number(x))) if *x < floor => Err(ResolveError::Invalid(format!(
            "{x} is less than the X the cost allows"
        ))),
        Some((_, Choice::Number(x))) => Ok(Announced::Price(at(template, *x), Some(*x))),
        None => Ok(Announced::Ask(Decision {
            player,
            kind: DecisionKind::ChooseNumber(NumberOptions {
                source: cx.scope.source.id,
                min: floor,
                max: Some(allowed(cx).max(floor)),
                except: Vec::new(),
                text: "Choose a value for X.".to_owned(),
                can_cancel: false,
                cost: None,
                per_x: 0,
                secret: false,
            }),
        })),
        Some((_, other)) => Err(ResolveError::Invalid(format!(
            "{other:?} is no value of X {player:?} can pay"
        ))),
    }
}

/// The least X the cost allows (`CostItem::XAtLeast`, "pay one or more {E}"): the greatest floor it names, 0 without
/// one.
pub(super) fn r4707_floor(cost: &mtg_cards::cost::Cost) -> u32 {
    cost.0
        .iter()
        .filter_map(|item| match item {
            mtg_cards::cost::CostItem::XAtLeast(least) => Some(*least),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// The most X the cost allows (`CostItem::XAtMost`, "X can't be greater than ..."), read as the payment is asked: the
/// least of its caps, never below 0; `None` when it has none.
pub(super) fn r306_cap(
    cx: &Cx<'_>,
    cost: &mtg_cards::cost::Cost,
) -> Result<Option<u32>, ResolveError> {
    let mut cap: Option<u32> = None;
    for item in &cost.0 {
        if let mtg_cards::cost::CostItem::XAtMost(most) = item {
            let most = u32::try_from(cx.scope.amount(most)?.max(0)).unwrap_or(u32::MAX);
            cap = Some(cap.map_or(most, |held| held.min(most)));
        }
    }
    Ok(cap)
}

/// The most X the player can pay for: affording is monotone in X, so a binary search finds it.
fn most(cx: &Cx<'_>, player: PlayerId, template: &Price) -> u32 {
    let (mut low, mut high) = (0, W6C_MOST_X);
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        if can_pay(cx, player, &at(template, mid)) {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    low
}

/// The name a step binds for the X its payment announced, which the effects after it read (`Resolving::w6c_paid_x`).
pub(super) fn bound(x: Option<u32>) -> Vec<(String, Vec<Target>)> {
    x.map(|x| (format!("{W6C_PAID_X}{x}"), Vec::new()))
        .into_iter()
        .collect()
}
