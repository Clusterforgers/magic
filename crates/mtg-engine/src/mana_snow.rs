//! The snow symbol `{S}` in a payment (CR 107.4h): one mana from a snow source pays it.
//!
//! Snow mana carries the `SpendOn::Snow` rider, so a floating snow mana is a restricted unit of the pool and a snow
//! source is an option with the rider. A cost with `n` `{S}` is paid as its other symbols plus `n` snow mana, each
//! spent on `{S}`; any snow mana may be that one, so the payments to try are the ways to pick `n` suppliers (a floating
//! unit or a source that adds one mana for a tap) and pay the rest of the cost without them.

use mtg_cards::spend::SpendOn;

use crate::mana::{ManaOption, ManaPool, ManaSource, Payment};
use crate::spend::{Restricted, Spend};

/// The most picks of suppliers tried for one cost.
const BUDGET: usize = 512;

/// One mana of snow that can pay `{S}`.
#[derive(Debug, Clone)]
enum Supplier {
    /// A floating unit of the pool, by its place among the restricted ones.
    Unit(usize),
    /// A source that adds one mana for a tap.
    Tap(ManaOption),
}

/// The pool and sources left once some snow mana is set aside for `{S}`, and the payment of that mana.
#[derive(Debug, Clone)]
pub(crate) struct Choice {
    /// The pool without the units set aside.
    pub pool: ManaPool,
    /// The sources without the permanents set aside.
    pub sources: Vec<ManaOption>,
    /// The part of the payment that pays `{S}`.
    pub paid: Payment,
}

impl Choice {
    /// `payment` with the snow mana that pays `{S}` added.
    pub(crate) fn join(&self, mut payment: Payment) -> Payment {
        payment.restricted.extend(self.paid.restricted.clone());
        payment.taps.extend(self.paid.taps.clone());
        payment
    }
}

/// Every way to set aside `count` snow mana that `spend` allows, up to a budget, the fewest taps first.
pub(crate) fn choices(
    pool: &ManaPool,
    sources: &[ManaOption],
    spend: &Spend,
    count: u32,
) -> Vec<Choice> {
    let mut suppliers: Vec<Supplier> = pool
        .restricted()
        .iter()
        .enumerate()
        .filter(|(_, unit)| is_snow(&unit.only) && spend.allows(&unit.only))
        .map(|(at, _)| Supplier::Unit(at))
        .collect();
    let mut permanents = Vec::new();
    for option in sources {
        let plain = option.is_simple()
            && is_snow(&option.only)
            && spend.allows(&option.only)
            && !permanents.contains(&option.source.object);
        if plain {
            permanents.push(option.source.object);
            suppliers.push(Supplier::Tap(option.clone()));
        }
    }
    let mut found = Vec::new();
    let mut picked = Vec::new();
    pick(&suppliers, 0, count as usize, &mut picked, &mut found);
    found
        .into_iter()
        .map(|picked| apply(pool, sources, &picked))
        .collect()
}

pub(crate) fn is_snow(only: &[SpendOn]) -> bool {
    only.contains(&SpendOn::Snow)
}

/// Collects the picks of `left` more suppliers from `all[from..]`.
fn pick(
    all: &[Supplier],
    from: usize,
    left: usize,
    picked: &mut Vec<Supplier>,
    found: &mut Vec<Vec<Supplier>>,
) {
    if found.len() >= BUDGET {
        return;
    }
    if left == 0 {
        found.push(picked.clone());
        return;
    }
    for at in from..all.len() {
        if all.len() - at < left {
            break;
        }
        picked.push(all[at].clone());
        pick(all, at + 1, left - 1, picked, found);
        picked.pop();
    }
}

fn apply(pool: &ManaPool, sources: &[ManaOption], picked: &[Supplier]) -> Choice {
    let mut units: Vec<usize> = Vec::new();
    let mut taps: Vec<ManaSource> = Vec::new();
    let mut objects = Vec::new();
    for supplier in picked {
        match supplier {
            Supplier::Unit(at) => units.push(*at),
            Supplier::Tap(option) => {
                taps.push(option.source);
                objects.push(option.source.object);
            }
        }
    }
    let restricted: Vec<Restricted> = units
        .iter()
        .filter_map(|at| pool.restricted().get(*at).cloned())
        .collect();
    Choice {
        pool: pool.without_restricted(&units),
        sources: sources
            .iter()
            .filter(|o| !objects.contains(&o.source.object))
            .cloned()
            .collect(),
        paid: Payment {
            restricted,
            taps,
            ..Payment::default()
        },
    }
}

/// How many of the mana a payment spends is snow mana: the restricted units and taps with the rider.
pub(crate) fn spent_snow(spent: &crate::spend::Spent) -> u32 {
    let count = spent
        .restricted
        .iter()
        .filter(|unit| is_snow(&unit.only))
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// How many snow mana the partial payment already spends.
pub(crate) fn held_snow(sources: &[ManaOption], partial: &Payment) -> u32 {
    let units = partial
        .restricted
        .iter()
        .filter(|unit| is_snow(&unit.only))
        .count();
    let taps: usize = partial
        .taps
        .iter()
        .filter_map(|tap| sources.iter().find(|o| o.source == *tap))
        .filter(|o| is_snow(&o.only))
        .map(|o| o.made().len())
        .sum();
    u32::try_from(units + taps).unwrap_or(u32::MAX)
}

/// The pool and sources without what the partial payment already uses.
pub(crate) fn without_partial(
    pool: &ManaPool,
    sources: &[ManaOption],
    partial: &Payment,
) -> (ManaPool, Vec<ManaOption>) {
    let mut left = pool.clone();
    for unit in &partial.restricted {
        left.spend_restricted(unit);
    }
    let used: Vec<_> = partial.taps.iter().map(|t| t.object).collect();
    let rest = sources
        .iter()
        .filter(|o| !used.contains(&o.source.object))
        .cloned()
        .collect();
    (left, rest)
}
