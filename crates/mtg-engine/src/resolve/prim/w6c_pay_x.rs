//! The `{X}` of a payment an effect asks for ("you may pay {X}{R}. If you do, it deals X damage"): a player who says yes
//! announces X (`ChooseNumber`, from 0 to the most they can pay), pays the cost with X generic mana for each `{X}`, and
//! the effects after the payment read that X (CR 107.3, 107.1c).

use std::iter::Peekable;

use mtg_core::ids::PlayerId;
use mtg_core::mana::{ManaCost, ManaSymbol};

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
        let generic = ManaSymbol::Generic(x.saturating_mul(price.x));
        price.mana = Some(price.mana.take().unwrap_or(ManaCost::ZERO).with(generic));
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
    journal: &mut Peekable<impl Iterator<Item = &'a (PlayerId, Choice)>>,
) -> Result<Announced, ResolveError> {
    if template.x == 0 && !template.counted() {
        return Ok(Announced::Price(template.clone(), None));
    }
    // The answer was checked against the most the player could pay when it was asked; the mana is spent since.
    match journal.next_if(|entry| entry.0 == player) {
        // A counted part is paid after the answer, so the answer is held to what the player holds now.
        Some((_, Choice::Number(x))) if template.x == 0 && *x > most(cx, player, template) => Err(
            ResolveError::Invalid(format!("{x} is more than {player:?} can pay")),
        ),
        Some((_, Choice::Number(x))) => Ok(Announced::Price(at(template, *x), Some(*x))),
        None => Ok(Announced::Ask(Decision {
            player,
            kind: DecisionKind::ChooseNumber(NumberOptions {
                source: cx.scope.source.id,
                min: 0,
                max: Some(most(cx, player, template)),
                except: Vec::new(),
                text: "Choose a value for X.".to_owned(),
                can_cancel: false,
                cost: None,
                per_x: 0,
            }),
        })),
        Some((_, other)) => Err(ResolveError::Invalid(format!(
            "{other:?} is no value of X {player:?} can pay"
        ))),
    }
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
