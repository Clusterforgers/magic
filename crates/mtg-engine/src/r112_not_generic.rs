//! Mana that can't be spent to pay generic mana costs (`SpendOn::R112NotGeneric`, CR 106.6): "{T}: Add {W}{U}{B}{R}{G}.
//! This mana can't be spent to pay generic mana costs."
//!
//! Such mana pays only a symbol of its own type: a colored symbol, the colored half of a hybrid, mono-hybrid or
//! Phyrexian one, and `{C}` for colorless mana. Before the plain-cost code runs, each payment sets that mana aside: the
//! floating units and the sources that add it (each source tapped or not) pay the symbols they can, and the rest of
//! the cost is paid as before by the other mana, which never sees the set-aside mana. A unit of a source that pays no
//! symbol stays in the pool unspent.

use mtg_cards::spend::{ManaLasts, SpendOn};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType};

use crate::error::PaymentError;
use crate::mana::{ManaOption, ManaPool, Payment};
use crate::spend::{Restricted, Spend, Spent, Step};

/// The most sources of such mana told apart; the others are not tapped for it.
const MOST_SOURCES: usize = 5;

/// Whether mana with these uses pays no generic cost.
pub(crate) fn r112_is_not_generic(only: &[SpendOn]) -> bool {
    only.contains(&SpendOn::R112NotGeneric)
}

/// Whether the pool or the sources hold such mana `spend` may use.
pub(crate) fn r112_present(pool: &ManaPool, sources: &[ManaOption], spend: &Spend) -> bool {
    let usable = |only: &[SpendOn]| r112_is_not_generic(only) && spend.allows(only);
    pool.restricted().iter().any(|unit| usable(&unit.only))
        || sources.iter().any(|option| usable(&option.only))
}

/// The cost left once such mana pays a symbol, and the pool and sources the rest of the payment draws on.
#[derive(Debug, Clone)]
pub(crate) struct R112Aside {
    /// The symbols the mana set aside does not pay.
    pub cost: ManaCost,
    /// The pool without any such mana.
    pub pool: ManaPool,
    /// The sources without any that adds such mana.
    pub sources: Vec<ManaOption>,
    /// The floating units spent and the sources tapped.
    pub paid: Payment,
}

impl R112Aside {
    /// `payment` with the mana set aside added, its taps first.
    pub(crate) fn join(&self, mut payment: Payment) -> Payment {
        payment.restricted.extend(self.paid.restricted.clone());
        let mut taps = self.paid.taps.clone();
        taps.extend(payment.taps);
        payment.taps = taps;
        payment
    }
}

/// The symbols a mana of `mana` pays, the best use first: its own colored symbol or `{C}`, then a hybrid symbol of
/// its color, then a Phyrexian one.
fn rank(symbol: ManaSymbol, mana: ManaType) -> Option<u8> {
    let color = |c: Color| ManaType::from(c) == mana;
    match symbol {
        ManaSymbol::Colored(c) if color(c) => Some(0),
        ManaSymbol::Colorless if mana == ManaType::Colorless => Some(0),
        ManaSymbol::Hybrid(a, b) | ManaSymbol::HybridPhyrexian(a, b) if color(a) || color(b) => {
            Some(1)
        }
        ManaSymbol::MonoHybrid(c) | ManaSymbol::ColorlessHybrid(c) if color(c) => Some(1),
        ManaSymbol::ColorlessHybrid(_) | ManaSymbol::ColorlessPhyrexian
            if mana == ManaType::Colorless =>
        {
            Some(1)
        }
        ManaSymbol::Phyrexian(c) if color(c) => Some(2),
        _ => None,
    }
}

/// Pays the symbols of `cost` with `units`, each unit the best symbol it can: the symbols left, and which units paid.
fn pay_symbols(cost: &ManaCost, units: &[ManaType]) -> (ManaCost, Vec<bool>) {
    let mut left: Vec<ManaSymbol> = cost.symbols().to_vec();
    let mut used = vec![false; units.len()];
    for (at, mana) in units.iter().enumerate() {
        let best = left
            .iter()
            .enumerate()
            .filter_map(|(place, symbol)| rank(*symbol, *mana).map(|r| (r, place)))
            .min();
        if let Some((_, place)) = best {
            left.remove(place);
            used[at] = true;
        }
    }
    (ManaCost::from_symbols(left), used)
}

/// The places of such units of the pool `spend` may use.
fn pool_places(pool: &ManaPool, spend: &Spend) -> Vec<usize> {
    pool.restricted()
        .iter()
        .enumerate()
        .filter(|(_, unit)| r112_is_not_generic(&unit.only) && spend.allows(&unit.only))
        .map(|(at, _)| at)
        .collect()
}

/// The pool and sources without such mana.
pub(crate) fn r112_without(
    pool: &ManaPool,
    sources: &[ManaOption],
    spend: &Spend,
) -> (ManaPool, Vec<ManaOption>) {
    (
        pool.without_restricted(&pool_places(pool, spend)),
        other_sources(sources),
    )
}

/// `payment` without the such mana it names: its floating units and the sources of it it taps.
pub(crate) fn r112_strip(payment: &Payment, sources: &[ManaOption]) -> Payment {
    let adds = |tap: &crate::mana::ManaSource| {
        sources
            .iter()
            .any(|option| option.source == *tap && r112_is_not_generic(&option.only))
    };
    Payment {
        restricted: payment
            .restricted
            .iter()
            .filter(|unit| !r112_is_not_generic(&unit.only))
            .cloned()
            .collect(),
        taps: payment.taps.iter().filter(|tap| !adds(tap)).copied().collect(),
        ..payment.clone()
    }
}

/// The sources without any that adds such mana.
fn other_sources(sources: &[ManaOption]) -> Vec<ManaOption> {
    sources
        .iter()
        .filter(|option| !r112_is_not_generic(&option.only))
        .cloned()
        .collect()
}

/// Every way to set such mana aside for `cost`: the floating units always, each source of it tapped or not (a source
/// whose mana pays no symbol is never tapped), the fewest taps first.
pub(crate) fn r112_choices(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    spend: &Spend,
) -> Vec<R112Aside> {
    let places = pool_places(pool, spend);
    let mut adders: Vec<&ManaOption> = Vec::new();
    for option in sources {
        if r112_is_not_generic(&option.only)
            && spend.allows(&option.only)
            && !option.is_costly()
            && option.takes.is_empty()
            && !adders
                .iter()
                .any(|held| held.source.object == option.source.object)
            && adders.len() < MOST_SOURCES
        {
            adders.push(option);
        }
    }
    let rest_pool = pool.without_restricted(&places);
    let rest_sources = other_sources(sources);
    let mut sets: Vec<u32> = (0..1u32 << adders.len()).collect();
    sets.sort_by_key(|set| set.count_ones());
    let mut found = Vec::new();
    for set in sets {
        let chosen: Vec<&ManaOption> = (0..adders.len())
            .filter(|at| set >> at & 1 == 1)
            .map(|at| adders[at])
            .collect();
        let mut units: Vec<(ManaType, Option<usize>)> = places
            .iter()
            .filter_map(|at| pool.restricted().get(*at).map(|unit| (unit.mana, Some(*at))))
            .collect();
        let mut owners = Vec::new();
        for (index, option) in chosen.iter().enumerate() {
            for mana in option.made() {
                units.push((mana, None));
                owners.push((units.len() - 1, index));
            }
        }
        let types: Vec<ManaType> = units.iter().map(|(mana, _)| *mana).collect();
        let (rest, used) = pay_symbols(cost, &types);
        // A tapped source must pay a symbol, or tapping it was for nothing.
        let idle = (0..chosen.len()).any(|index| {
            !owners
                .iter()
                .any(|(unit, owner)| *owner == index && used[*unit])
        });
        if idle {
            continue;
        }
        let restricted = units
            .iter()
            .zip(&used)
            .filter_map(|((_, place), paid)| {
                place
                    .filter(|_| *paid)
                    .and_then(|at| pool.restricted().get(at).cloned())
            })
            .collect();
        found.push(R112Aside {
            cost: rest,
            pool: rest_pool.clone(),
            sources: rest_sources.clone(),
            paid: Payment {
                restricted,
                taps: chosen.iter().map(|option| option.source).collect(),
                ..Payment::default()
            },
        });
    }
    found
}

/// A payment that names such mana, told apart: the cost its other mana pays, the pool and sources that mana comes
/// from, the payment without such mana, and what such mana spends.
#[derive(Debug, Clone)]
pub(crate) struct R112Split {
    /// The symbols such mana does not pay.
    pub cost: ManaCost,
    /// The pool without any such mana.
    pub pool: ManaPool,
    /// The sources without any that adds such mana.
    pub sources: Vec<ManaOption>,
    /// The payment without such mana.
    pub payment: Payment,
    /// The mana such mana spends and the sources the payment taps for it.
    pub spent: Spent,
}

impl R112Split {
    /// `got`, the mana the rest of the payment spends, with such mana added, its taps first.
    pub(crate) fn join(self, mut got: Spent) -> Spent {
        got.mana.extend(self.spent.mana);
        got.restricted.extend(self.spent.restricted);
        let mut steps = self.spent.steps;
        steps.extend(got.steps);
        got.steps = steps;
        got
    }
}

/// Tells apart the such mana `payment` names, or `None` when it names none. A floating unit that pays no symbol is
/// refused (it would be wasted); a tapped source's unit that pays none stays in the pool.
pub(crate) fn r112_split(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    tapped: &[ObjectId],
    spend: &Spend,
    payment: &Payment,
) -> Result<Option<R112Split>, PaymentError> {
    let named_units: Vec<&Restricted> = payment
        .restricted
        .iter()
        .filter(|unit| r112_is_not_generic(&unit.only))
        .collect();
    let named_taps: Vec<&ManaOption> = payment
        .taps
        .iter()
        .filter_map(|tap| sources.iter().find(|option| option.source == *tap))
        .filter(|option| r112_is_not_generic(&option.only))
        .collect();
    if named_units.is_empty() && named_taps.is_empty() {
        return Ok(None);
    }
    let mut left = pool.clone();
    for unit in &named_units {
        if !spend.allows(&unit.only) {
            return Err(PaymentError::Restricted(unit.mana));
        }
        if !left.spend_restricted(unit) {
            return Err(PaymentError::PoolShortfall(unit.mana));
        }
    }
    for option in &named_taps {
        if !spend.allows(&option.only) {
            return Err(PaymentError::Restricted(option.produces));
        }
        if tapped.contains(&option.source.object) {
            return Err(PaymentError::AlreadyTapped(option.source.object));
        }
    }
    let mut units: Vec<Restricted> = named_units.iter().map(|unit| (*unit).clone()).collect();
    let floating = units.len();
    for option in &named_taps {
        units.extend(option.made().into_iter().map(|mana| Restricted {
            mana,
            only: option.only.clone(),
            lasts: ManaLasts::Step,
        }));
    }
    let types: Vec<ManaType> = units.iter().map(|unit| unit.mana).collect();
    let (rest, used) = pay_symbols(cost, &types);
    if used[..floating].iter().any(|paid| !paid) {
        return Err(PaymentError::Wasted);
    }
    let spent_units: Vec<Restricted> = units
        .into_iter()
        .zip(&used)
        .filter(|(_, paid)| **paid)
        .map(|(unit, _)| unit)
        .collect();
    let places = pool_places(pool, spend);
    Ok(Some(R112Split {
        cost: rest,
        pool: pool.without_restricted(&places),
        sources: other_sources(sources),
        payment: r112_strip(payment, sources),
        spent: Spent {
            mana: spent_units.iter().map(|unit| unit.mana).collect(),
            restricted: spent_units,
            steps: named_taps
                .iter()
                .map(|option| Step {
                    source: option.source,
                    paid: Vec::new(),
                })
                .collect(),
        },
    }))
}

#[cfg(test)]
mod tests {
    //! Goal: mana that can't pay generic costs pays each colored, hybrid, Phyrexian and `{C}` symbol of its type and
    //! never a generic one, whatever the cost and the order of the units.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use rstest::rstest;

    const WUBRG: [ManaType; 5] = [
        ManaType::White,
        ManaType::Blue,
        ManaType::Black,
        ManaType::Red,
        ManaType::Green,
    ];

    #[rstest]
    #[case::generic("{3}", "{3}", 0)]
    #[case::one_of_each("{W}{U}{B}{R}{G}", "{0}", 5)]
    #[case::red_and_generic("{2}{R}", "{2}", 1)]
    #[case::two_red("{R}{R}", "{R}", 1)]
    #[case::hybrid("{W/U}{2/B}{G/P}", "{0}", 3)]
    #[case::generic_stays("{4}{G}{G}", "{4}{G}", 1)]
    #[case::colorless("{C}", "{C}", 0)]
    fn r112_wubrg_pays_only_symbols_of_its_types(
        #[case] cost: &str,
        #[case] rest: &str,
        #[case] paid: usize,
    ) {
        let (left, used) = pay_symbols(&ManaCost::parse(cost).unwrap(), &WUBRG);
        assert_eq!(left, ManaCost::parse(rest).unwrap(), "{cost}");
        assert_eq!(used.iter().filter(|u| **u).count(), paid, "{cost}");
    }

    #[test]
    fn r112_colored_symbol_before_its_hybrid() {
        let (left, _) = pay_symbols(&ManaCost::parse("{R/G}{R}").unwrap(), &[ManaType::Red]);
        assert_eq!(left, ManaCost::parse("{R/G}").unwrap());
        let (left, _) = pay_symbols(&ManaCost::parse("{C}{1}").unwrap(), &[ManaType::Colorless]);
        assert_eq!(left, ManaCost::parse("{1}").unwrap());
    }
}
