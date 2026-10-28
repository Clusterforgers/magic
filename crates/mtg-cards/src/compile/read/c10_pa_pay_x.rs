//! "You may pay X life, where X is the amount of life you gained this turn. If you do, create an X/X ... token": a
//! payment of life counted as the ability resolves, with the same count in the effects of the payment. The writer is
//! the generic one of `where_x::wordings`, which names the one count of the ability as X, and `c10_pa_wordings` when an
//! intervening "if" compares the same count.

use super::tokens::Reader;
use super::{defining, render, where_x};
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::effect::Effect;
use crate::value::Amount;

/// After "pay": "X life, where X is ..." up to the period; the amount X names. X then reads in the sentence after "If you
/// do" (`Reader::where_x`) until `c10_pa_put_in` gives it the amount.
pub(super) fn c10_pa_x_life(r: &mut Reader<'_>) -> Option<(Cost, Amount)> {
    r.word("x").then_some(())?;
    r.word("life").then_some(())?;
    let amount = where_x::amount(r)?;
    // A count the payer chooses is not one the resolution counts.
    (!matches!(amount, Amount::X | Amount::Fixed(_))).then_some(())?;
    r.where_x = true;
    Some((Cost(vec![CostItem::PayLife(amount.clone())]), amount))
}

/// The wordings of an ability whose one effect is a payment of counted life, when a condition of the ability compares
/// the same count ("if you gained life this turn, you may pay X life, where X is the amount of life you gained this
/// turn"), which `where_x::wordings` leaves alone: X stands for the count in the payment only.
pub(super) fn c10_pa_wordings(ability: &Ability) -> Vec<String> {
    let [Effect::Pay { cost, .. }] = ability.effects() else {
        return Vec::new();
    };
    let [CostItem::PayLife(amount)] = cost.0.as_slice() else {
        return Vec::new();
    };
    if matches!(amount, Amount::X | Amount::Fixed(_)) {
        return Vec::new();
    }
    let mut replaced = ability.clone();
    let Ability::Triggered {
        intervening_if: Some(_),
        effects,
        ..
    } = &mut replaced
    else {
        return Vec::new();
    };
    let Some(with_x) = super::c10_pa_monstrous::c10_pa_swap(effects, amount, &Amount::X) else {
        return Vec::new();
    };
    *effects = with_x;
    let mut found = Vec::new();
    for said in defining::value_texts(amount) {
        for text in render::variants(&replaced) {
            if let Some(at) = text.find("pay X life.") {
                let (head, tail) = text.split_at(at + "pay X life".len());
                found.push(format!("{head}, where X is {said}{tail}"));
            }
        }
    }
    found
}

/// The effects of the payment with the amount in the place of X; the where clause is read, so the sentence ends without
/// one. Effects without an X keep their amounts.
pub(super) fn c10_pa_put_in(r: &mut Reader<'_>, paid: Vec<Effect>, amount: &Amount) -> Vec<Effect> {
    r.where_x = false;
    where_x::put_in(&paid, amount).unwrap_or(paid)
}
