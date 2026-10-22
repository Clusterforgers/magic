//! Mana abilities that cost mana (a Signet's `{1}, {T}: Add {W}{U}`) inside a payment.
//!
//! The mana such an ability costs is paid from the pool as it is activated (CR 605.3b), so it is mana already there:
//! floating mana, or mana that sources activated before it added. A payment is valid when its sources can be put in an
//! order in which every cost is paid so and the mana left covers the cost with nothing to spare ([`spent`]). A source
//! never pays for itself and one that needs another's mana comes after it, so no two need each other. An automatic
//! payment ([`auto`]) searches the order backwards: the source activated last covers some of the cost and adds its own
//! cost to what is still needed, the sources before it cover that, and plain sources and floating mana come first. At
//! most [`CHAIN`] sources that cost mana are used in one payment it chooses.
//!
//! Mana with a restriction can pay the cost of a spell or ability but never the mana cost of a mana ability, so it is
//! applied to the cost of the payment before any of those costs is added.

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::mana::{ManaCost, ManaType};

use crate::error::PaymentError;
use crate::mana::{Floating, ManaOption, ManaSource, Payment};
use crate::mana_plan::{Needs, Weight, select};
use crate::spend::{Restricted, Step};

/// The most sources that cost mana an automatic payment uses.
pub(crate) const CHAIN: usize = 4;

/// The most nodes a search for the order of sources looks at before it gives up.
const BUDGET: usize = 50_000;

/// Mana by type (`ManaType::index`).
type Counts = [u32; 6];

fn counts(types: &[ManaType]) -> Counts {
    let mut found: Counts = [0; 6];
    for mana in types {
        found[mana.index()] += 1;
    }
    found
}

fn total(counts: &Counts) -> u32 {
    counts.iter().sum()
}

/// What one way to pay a plain mana cost asks: mana of each type, and more mana of any type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ask {
    typed: Counts,
    any: u32,
}

/// Every way to pay the mana of a cost, as the plain costs its hybrid symbols resolve to.
fn asks(cost: &ManaCost) -> Vec<Ask> {
    cost.resolutions()
        .into_iter()
        .filter(|resolution| resolution.life == 0)
        .map(|resolution| {
            let mut typed: Counts = [0; 6];
            for mana in ManaType::ALL {
                typed[mana.index()] = resolution.cost.required(mana);
            }
            Ask {
                typed,
                any: resolution.cost.generic(),
            }
        })
        .collect()
}

/// The mana one use of `option` adds, apart into what has no restriction and what has one.
fn made_by(option: &ManaOption) -> (Counts, Counts) {
    let made = counts(&option.made());
    if option.only.is_empty() {
        (made, [0; 6])
    } else {
        ([0; 6], made)
    }
}

/// The mana in the pool before a payment's sources are used, and the sources.
pub(crate) struct Supply<'a> {
    /// Floating mana without a restriction.
    pub free: Counts,
    /// Floating mana with one, which no cost of a mana ability may take.
    pub locked: Counts,
    /// The sources used.
    pub taps: Vec<&'a ManaOption>,
}

/// An order of the sources of a payment in which every mana cost is paid, and the mana left.
pub(crate) struct Schedule {
    /// The sources in the order they are activated, with what their costs take.
    pub steps: Vec<Step>,
    /// Mana left without a restriction.
    pub free: Counts,
    /// Mana left with one.
    pub locked: Counts,
}

/// Whether `free` and `locked` mana cover the plain `cost`: enough in all and of each type.
fn covers(cost: &ManaCost, free: &Counts, locked: &Counts) -> bool {
    let value = cost.mana_value();
    total(free) + total(locked) >= value
        && ManaType::ALL
            .into_iter()
            .all(|mana| free[mana.index()] + locked[mana.index()] >= cost.required(mana))
}

/// Every way to take `n` more mana of any type out of `avail`, mana of the types `order` lists first taken first.
fn spread(
    n: u32,
    order: &[usize],
    at: usize,
    avail: &Counts,
    taken: &mut Counts,
    out: &mut Vec<Counts>,
) {
    if n == 0 {
        out.push(*taken);
        return;
    }
    let Some(&kind) = order.get(at) else {
        return;
    };
    for count in (0..=n.min(avail[kind])).rev() {
        taken[kind] = count;
        spread(n - count, order, at + 1, avail, taken, out);
    }
    taken[kind] = 0;
}

/// The search for the order of the sources that cost mana.
struct Search<'a> {
    cost: &'a ManaCost,
    costly: Vec<&'a ManaOption>,
    asks: Vec<Vec<Ask>>,
    seen: BTreeSet<(u32, Counts)>,
    steps: Vec<Step>,
    budget: usize,
}

impl Search<'_> {
    /// Activates the sources not yet in `done`, the free mana being `free` and the locked mana `locked`; the mana left
    /// when all of them were activated and the cost is covered.
    fn go(&mut self, done: u32, free: Counts, locked: Counts) -> Option<(Counts, Counts)> {
        if done.count_ones() as usize == self.costly.len() {
            return covers(self.cost, &free, &locked).then_some((free, locked));
        }
        if self.budget == 0 || !self.seen.insert((done, free)) {
            return None;
        }
        self.budget -= 1;
        // Mana the cost needs least is taken first for a cost of any type.
        let mut order: Vec<usize> = (0..6).collect();
        order.sort_by_key(|&kind| {
            let required = i64::from(self.cost.required(ManaType::ALL[kind]));
            std::cmp::Reverse(i64::from(free[kind] + locked[kind]) - required)
        });
        for at in 0..self.costly.len() {
            if done & (1 << at) != 0 {
                continue;
            }
            let option = self.costly[at];
            let (made_free, made_locked) = made_by(option);
            for ask in self.asks[at].clone() {
                if (0..6).any(|kind| ask.typed[kind] > free[kind]) {
                    continue;
                }
                let mut after = free;
                for (slot, typed) in after.iter_mut().zip(ask.typed) {
                    *slot -= typed;
                }
                let mut ways = Vec::new();
                spread(ask.any, &order, 0, &after, &mut [0; 6], &mut ways);
                for taken in ways {
                    let mut next = after;
                    let mut next_locked = locked;
                    let mut paid = Vec::new();
                    for kind in 0..6 {
                        next[kind] = next[kind] - taken[kind] + made_free[kind];
                        next_locked[kind] += made_locked[kind];
                        let units = ask.typed[kind] + taken[kind];
                        paid.extend(std::iter::repeat_n(ManaType::ALL[kind], units as usize));
                    }
                    self.steps.push(Step {
                        source: option.source,
                        paid,
                    });
                    if let Some(found) = self.go(done | (1 << at), next, next_locked) {
                        return Some(found);
                    }
                    self.steps.pop();
                }
            }
        }
        None
    }
}

/// An order for the sources of `supply` in which each pays its mana cost from the pool and the mana left covers the
/// plain `cost`; sources that cost nothing come first. `None` when there is none.
pub(crate) fn schedule(cost: &ManaCost, supply: &Supply<'_>) -> Option<Schedule> {
    let (mut free, mut locked) = (supply.free, supply.locked);
    let mut steps = Vec::new();
    let mut costly = Vec::new();
    for &option in &supply.taps {
        if option.is_costly() {
            costly.push(option);
        } else {
            let (made_free, made_locked) = made_by(option);
            for kind in 0..6 {
                free[kind] += made_free[kind];
                locked[kind] += made_locked[kind];
            }
            steps.push(Step {
                source: option.source,
                paid: Vec::new(),
            });
        }
    }
    if costly.len() > 31 {
        return None;
    }
    let mut search = Search {
        cost,
        asks: costly.iter().map(|o| asks(&o.cost)).collect(),
        costly,
        seen: BTreeSet::new(),
        steps,
        budget: BUDGET,
    };
    let (free, locked) = search.go(0, free, locked)?;
    Some(Schedule {
        steps: search.steps,
        free,
        locked,
    })
}

/// The parts of a payment: its floating mana and the sources it taps.
fn supply_of<'a>(
    pool: &[ManaType],
    restricted: &[Restricted],
    taps: &[&'a ManaOption],
) -> Supply<'a> {
    Supply {
        free: counts(pool),
        locked: counts(&restricted.iter().map(|unit| unit.mana).collect::<Vec<_>>()),
        taps: taps.to_vec(),
    }
}

/// The options a payment taps, in the order it names them.
fn taps_of<'a>(payment: &Payment, sources: &'a [ManaOption]) -> Vec<&'a ManaOption> {
    payment
        .taps
        .iter()
        .filter_map(|tap| sources.iter().find(|o| o.source == *tap))
        .collect()
}

/// The mana `payment` spends on the plain `cost` when some source it taps costs mana, and the order of its sources.
///
/// The payment must have an order in which every cost is paid ([`schedule`]) with the mana left covering `cost`, and
/// leave no part to spare: floating mana or a source that could be left out and still cover it is refused.
pub(crate) fn spent(
    cost: &ManaCost,
    payment: &Payment,
    sources: &[ManaOption],
) -> Result<(Vec<ManaType>, Vec<Step>), PaymentError> {
    let options = taps_of(payment, sources);
    let held = supply_of(&payment.pool, &payment.restricted, &options);
    let value = cost.mana_value();
    let Some(found) = schedule(cost, &held) else {
        return Err(refusal(cost, payment, &options));
    };
    let wasted = || PaymentError::WrongTotal {
        needed: value,
        got: total(&found.free) + total(&found.locked),
    };
    for at in 0..payment.pool.len() {
        let mut pool = payment.pool.clone();
        pool.remove(at);
        if schedule(cost, &supply_of(&pool, &payment.restricted, &options)).is_some() {
            return Err(wasted());
        }
    }
    for at in 0..payment.restricted.len() {
        let mut restricted = payment.restricted.clone();
        restricted.remove(at);
        if schedule(cost, &supply_of(&payment.pool, &restricted, &options)).is_some() {
            return Err(wasted());
        }
    }
    for at in 0..options.len() {
        let mut rest = options.clone();
        rest.remove(at);
        if schedule(cost, &supply_of(&payment.pool, &payment.restricted, &rest)).is_some() {
            return Err(wasted());
        }
    }
    let mut left: Vec<ManaType> = Vec::new();
    for mana in ManaType::ALL {
        left.extend(std::iter::repeat_n(mana, found.free[mana.index()] as usize));
    }
    for mana in ManaType::ALL {
        left.extend(std::iter::repeat_n(
            mana,
            found.locked[mana.index()] as usize,
        ));
    }
    Ok((select(cost, &left), found.steps))
}

/// Why a payment with sources that cost mana has no order that pays: too little mana, a missing type, or a cost that
/// the mana made before its source cannot pay.
fn refusal(cost: &ManaCost, payment: &Payment, options: &[&ManaOption]) -> PaymentError {
    let mut made: Vec<ManaType> = payment.pool.clone();
    made.extend(payment.restricted.iter().map(|unit| unit.mana));
    let mut spent_on_costs = 0;
    for option in options {
        made.extend(option.made());
        spent_on_costs += asks(&option.cost)
            .iter()
            .map(|ask| total(&ask.typed) + ask.any)
            .min()
            .unwrap_or(0);
    }
    let value = cost.mana_value();
    let got = u32::try_from(made.len())
        .unwrap_or(u32::MAX)
        .saturating_sub(spent_on_costs);
    if got < value {
        return PaymentError::WrongTotal { needed: value, got };
    }
    let have = counts(&made);
    if let Some(missing) = ManaType::ALL
        .into_iter()
        .find(|mana| have[mana.index()] < cost.required(*mana))
    {
        return PaymentError::MissingMana(missing);
    }
    options
        .iter()
        .find(|option| option.is_costly())
        .map_or(PaymentError::Wasted, |option| {
            PaymentError::CostUnpayable(option.source)
        })
}

/// The parts of a payment that an automatic payment or a completion keeps.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Forced<'a> {
    /// Floating mana without a restriction.
    pub pool: &'a [ManaType],
    /// Floating mana with one.
    pub restricted: &'a [Restricted],
    /// Sources tapped.
    pub taps: &'a [ManaSource],
}

/// What is still needed of the plain cost and of the mana costs added to it: each type's requirement and more mana of any
/// kind, and which sources that cost mana or add restricted mana are used (bit `n` is the `n`th of them).
type Key = (u64, Needs, u32);

/// A plan's weight, the sources it taps and the floating mana it spends.
#[derive(Debug, Clone)]
struct Plan {
    weight: Weight,
    taps: Vec<ManaSource>,
    pool: Counts,
}

/// What is needed after mana `made` is applied to `need`: each unit meets a need of its type, else a need of any kind;
/// what neither takes floats.
fn apply(need: (Needs, u32), made: &[ManaType]) -> (Needs, u32) {
    let (mut needs, mut any) = need;
    for mana in made {
        let slot = &mut needs[mana.index()];
        if *slot > 0 {
            *slot -= 1;
        } else {
            any = any.saturating_sub(1);
        }
    }
    (needs, any)
}

/// What is needed after one `mana` is applied, `None` when there is nothing it could pay.
fn apply_one(need: (Needs, u32), mana: ManaType) -> Option<(Needs, u32)> {
    let (mut needs, mut any) = need;
    let slot = &mut needs[mana.index()];
    if *slot > 0 {
        *slot -= 1;
    } else if any > 0 {
        any -= 1;
    } else {
        return None;
    }
    Some((needs, any))
}

/// Applies as much of `units` as `need` takes, types first; the units used and the rest of the need.
fn apply_pool(
    need: (Needs, u32),
    units: &Counts,
    forced: &Counts,
) -> Option<((Needs, u32), Counts)> {
    let mut left = *units;
    let mut used: Counts = [0; 6];
    let mut need = need;
    // What a payment names has to pay for something.
    for kind in 0..6 {
        for _ in 0..forced[kind] {
            if left[kind] == 0 {
                return None;
            }
            need = apply_one(need, ManaType::ALL[kind])?;
            left[kind] -= 1;
            used[kind] += 1;
        }
    }
    for pass in 0..2 {
        for kind in 0..6 {
            while left[kind] > 0 {
                let typed = need.0[kind] > 0;
                if (pass == 0) != typed {
                    break;
                }
                let Some(after) = apply_one(need, ManaType::ALL[kind]) else {
                    break;
                };
                need = after;
                left[kind] -= 1;
                used[kind] += 1;
            }
        }
    }
    Some((need, used))
}

/// Puts the mana of the types in `types` into floating mana, `n` of each.
fn units_of(types: &Counts) -> Vec<ManaType> {
    let mut units = Vec::new();
    for mana in ManaType::ALL {
        units.extend(std::iter::repeat_n(mana, types[mana.index()] as usize));
    }
    units
}

/// The plans found so far, the best for each state.
type Plans = BTreeMap<Key, Plan>;

/// What a source does for a plan, which decides the stage that uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Adds mana with a restriction, which pays the cost of the payment only.
    Restricted,
    /// Costs mana.
    Costly,
    /// Adds mana without a restriction and costs none.
    Plain,
}

fn class_of(option: &ManaOption) -> Class {
    if option.is_costly() {
        Class::Costly
    } else if option.only.is_empty() {
        Class::Plain
    } else {
        Class::Restricted
    }
}

/// The sources a plan chooses from, and the parts it must keep.
struct Planner<'a> {
    groups: &'a [Vec<ManaOption>],
    forced: &'a Forced<'a>,
    /// The groups that matter to more than one stage: sources that cost mana or add restricted mana.
    special: Vec<usize>,
    /// How many options there are, for the weight of a plan.
    all: u32,
}

impl Planner<'_> {
    /// The bit of `group` in the mask of the groups used, for a special group.
    fn bit_of(&self, group: usize) -> Option<u64> {
        self.special
            .iter()
            .position(|g| *g == group)
            .map(|n| 1u64 << n)
    }

    /// The group that has `source`.
    fn group_of(&self, source: &ManaSource) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.iter().any(|o| o.source == *source))
    }

    /// The source that was given for `group`, which is then the only way its permanent is used.
    fn pinned(&self, group: usize) -> Option<&ManaSource> {
        self.forced
            .taps
            .iter()
            .find(|tap| self.groups[group].iter().any(|o| o.source == **tap))
    }

    /// How many options make `mana`.
    fn plenty(&self, mana: ManaType) -> u32 {
        let found = self.groups.iter().flatten().filter(|o| o.produces == mana);
        u32::try_from(found.count()).unwrap_or(u32::MAX)
    }

    /// Adds the plan of `plan` with `option` used to `next` under `key`, if it is the best so far.
    fn insert(&self, next: &mut Plans, key: Key, plan: &Plan, option: &ManaOption) {
        let weight = plan
            .weight
            .with(option, self.all, self.plenty(option.produces));
        if next.get(&key).is_none_or(|held| weight < held.weight) {
            let mut taps = plan.taps.clone();
            taps.push(option.source);
            next.insert(
                key,
                Plan {
                    weight,
                    taps,
                    pool: plan.pool,
                },
            );
        }
    }

    /// Each group's options of `class` used or not, at most one option of a group; a source that was given is used.
    fn free_stage(&self, mut best: Plans, class: Class) -> Plans {
        for (at, group) in self.groups.iter().enumerate() {
            let bit = self.bit_of(at);
            if class == Class::Restricted && bit.is_none() {
                continue;
            }
            let must = self.pinned(at).filter(|tap| {
                group
                    .iter()
                    .any(|o| o.source == **tap && class_of(o) == class)
            });
            let mut next = if must.is_some() {
                Plans::new()
            } else {
                best.clone()
            };
            for ((mask, needs, left), plan) in &best {
                if class == Class::Plain && bit.is_some_and(|bit| mask & bit != 0) {
                    continue;
                }
                for option in group {
                    if class_of(option) != class
                        || self.pinned(at).is_some_and(|tap| option.source != *tap)
                    {
                        continue;
                    }
                    let after = apply((*needs, *left), &option.made());
                    let used = if class == Class::Restricted {
                        mask | bit.unwrap_or(0)
                    } else {
                        *mask
                    };
                    self.insert(&mut next, (used, after.0, after.1), plan, option);
                }
            }
            best = next;
        }
        best
    }

    /// Up to `depth` sources that cost mana, the last one activated first: each covers some of what is needed and adds its
    /// own cost to it. A plan that asks for more than the floating mana (`held`) and every source could make is dropped.
    fn costly_stage(&self, mut best: Plans, depth: usize, held: u32) -> Plans {
        let can_make: u32 = self
            .groups
            .iter()
            .map(|group| {
                let sizes = group.iter().map(|o| o.made().len());
                u32::try_from(sizes.max().unwrap_or(0)).unwrap_or(u32::MAX)
            })
            .sum();
        let mut layer = best.clone();
        for _ in 0..depth {
            let mut next = Plans::new();
            for ((mask, needs, left), plan) in &layer {
                for &group in &self.special {
                    let Some(bit) = self.bit_of(group) else {
                        continue;
                    };
                    if mask & bit != 0 {
                        continue;
                    }
                    for option in &self.groups[group] {
                        if class_of(option) != Class::Costly
                            || !option.only.is_empty()
                            || self.pinned(group).is_some_and(|tap| option.source != *tap)
                        {
                            continue;
                        }
                        for ask in asks(&option.cost) {
                            let (mut after, more) = apply((*needs, *left), &option.made());
                            for (slot, typed) in after.iter_mut().zip(ask.typed) {
                                *slot += typed;
                            }
                            let more = more + ask.any;
                            if total(&after) + more > held + can_make {
                                continue;
                            }
                            self.insert(&mut next, (mask | bit, after, more), plan, option);
                        }
                    }
                }
            }
            for (key, plan) in &next {
                if best.get(key).is_none_or(|kept| plan.weight < kept.weight) {
                    best.insert(*key, plan.clone());
                }
            }
            layer = next;
        }
        best
    }
}

/// The restricted floating mana that pays the cost of the payment, `need` after it, and the units used: the units in
/// `forced` first, then every other unit that meets a need, types first.
fn apply_restricted(
    mut need: (Needs, u32),
    pool: &Floating,
    forced: &Forced<'_>,
) -> Option<((Needs, u32), Vec<Restricted>)> {
    let mut units: Vec<Restricted> = pool.units().to_vec();
    let mut used: Vec<Restricted> = Vec::new();
    for unit in forced.restricted {
        let at = units.iter().position(|held| held == unit)?;
        need = apply_one(need, unit.mana)?;
        used.push(units.remove(at));
    }
    for pass in 0..2 {
        let mut rest = Vec::new();
        for unit in units {
            let typed = need.0[unit.mana.index()] > 0;
            match apply_one(need, unit.mana) {
                Some(after) if (pass == 0) == typed => {
                    need = after;
                    used.push(unit);
                }
                _ => rest.push(unit),
            }
        }
        units = rest;
    }
    Some((need, used))
}

/// The plan with the fewest sacrifices, then the fewest sources that do more than add mana, then the fewest taps, that
/// covers the plain `cost` and keeps the parts in `forced`; it may have a part to spare. `None` when the sources and
/// the floating mana cannot cover the cost.
///
/// The sources are searched backwards: floating mana with a restriction is applied to the cost first, then the sources
/// that add restricted mana, up to [`CHAIN`] sources that cost mana, the floating mana without a restriction and the
/// sources that cost nothing.
fn plan(
    cost: &ManaCost,
    pool: &Floating,
    groups: &[Vec<ManaOption>],
    forced: &Forced<'_>,
) -> Option<Payment> {
    let mut needs: Needs = [0; 6];
    for mana in ManaType::ALL {
        needs[mana.index()] = cost.required(mana);
    }
    let special: Vec<usize> = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group.iter().any(|o| o.is_costly() || !o.only.is_empty()))
        .map(|(at, _)| at)
        .take(64)
        .collect();
    let planner = Planner {
        groups,
        forced,
        special,
        all: u32::try_from(groups.iter().map(Vec::len).sum::<usize>()).unwrap_or(u32::MAX),
    };

    let (need, restricted) = apply_restricted((needs, cost.generic()), pool, forced)?;
    let mut best = Plans::new();
    best.insert(
        (0, need.0, need.1),
        Plan {
            weight: Weight::default(),
            taps: Vec::new(),
            pool: [0; 6],
        },
    );
    best = planner.free_stage(best, Class::Restricted);

    let mut must_mask = 0u64;
    let mut forced_costly = 0;
    for tap in forced.taps {
        let costly = groups
            .iter()
            .flatten()
            .any(|o| o.source == *tap && o.is_costly());
        if costly {
            forced_costly += 1;
            must_mask |= planner.group_of(tap).and_then(|g| planner.bit_of(g))?;
        }
    }
    best = planner.costly_stage(best, CHAIN.max(forced_costly), total(&pool.free_counts()));
    best.retain(|(mask, ..), _| mask & must_mask == must_mask);

    // Floating mana without a restriction, then the sources that cost nothing.
    let forced_pool = counts(forced.pool);
    let mut stage = Plans::new();
    for ((mask, needs, left), plan) in best {
        let Some(((needs, left), used)) =
            apply_pool((needs, left), &pool.free_counts(), &forced_pool)
        else {
            continue;
        };
        let key = (mask, needs, left);
        if stage.get(&key).is_none_or(|kept| plan.weight < kept.weight) {
            stage.insert(key, Plan { pool: used, ..plan });
        }
    }
    let chosen = planner
        .free_stage(stage, Class::Plain)
        .into_iter()
        .filter(|((_, needs, left), _)| *needs == [0; 6] && *left == 0)
        .map(|(_, plan)| plan)
        .min_by_key(|plan| plan.weight)?;

    let sources: Vec<ManaOption> = groups.iter().flatten().cloned().collect();
    let payment = trimmed(
        cost,
        Payment {
            pool: units_of(&chosen.pool),
            restricted,
            taps: chosen.taps,
            ..Payment::default()
        },
        forced,
        &sources,
    );
    // Every part that was given stays.
    let count = |units: &[ManaType], mana: ManaType| units.iter().filter(|m| **m == mana).count();
    let pool_kept = ManaType::ALL
        .into_iter()
        .all(|mana| count(&payment.pool, mana) >= count(forced.pool, mana));
    (pool_kept
        && forced.taps.iter().all(|tap| payment.taps.contains(tap))
        && forced
            .restricted
            .iter()
            .all(|unit| payment.restricted.contains(unit)))
    .then_some(payment)
}

/// `payment` without the floating mana that other parts cover, apart from what `forced` names: a source of several mana
/// may cover what floating mana was spent on, and a payment has nothing to spare.
fn trimmed(
    cost: &ManaCost,
    mut payment: Payment,
    forced: &Forced<'_>,
    sources: &[ManaOption],
) -> Payment {
    let mut at = payment.pool.len();
    while at > 0 {
        at -= 1;
        let mana = payment.pool[at];
        let count = |units: &[ManaType]| units.iter().filter(|m| **m == mana).count();
        if count(&payment.pool) <= count(forced.pool) {
            continue;
        }
        let mut without = payment.clone();
        without.pool.remove(at);
        if is_covered(cost, &without, sources) {
            payment = without;
        }
    }
    let mut at = payment.restricted.len();
    while at > 0 {
        at -= 1;
        if forced.restricted.contains(&payment.restricted[at]) {
            continue;
        }
        let mut without = payment.clone();
        without.restricted.remove(at);
        if is_covered(cost, &without, sources) {
            payment = without;
        }
    }
    payment
}

/// Chooses floating mana and sources that pay the plain `cost` with nothing to spare, keeping the parts in `forced`.
///
/// The plan [`plan`] finds is used when it is a payment; a source that adds several mana can leave a part of it to
/// spare, and then every combination is looked at ([`exhaustive`]).
pub(crate) fn auto(
    cost: &ManaCost,
    pool: &Floating,
    groups: &[Vec<ManaOption>],
    forced: &Forced<'_>,
) -> Option<Payment> {
    let planned = plan(cost, pool, groups, forced)?;
    let sources: Vec<ManaOption> = groups.iter().flatten().cloned().collect();
    if spent(cost, &planned, &sources).is_ok() {
        return Some(planned);
    }
    exhaustive(cost, pool, groups, forced, &sources)
}

/// What an exhaustive search may add to a payment: one floating mana, or one permanent used for one of its options.
enum Candidate<'a> {
    /// A floating mana without a restriction of this type, and how many there are.
    Pool(ManaType, usize),
    /// A floating mana with a restriction.
    Restricted(&'a Restricted),
    /// A permanent not yet tapped.
    Permanent(&'a [ManaOption]),
}

/// The best payment found so far: its weight (sacrifices, side effects, taps, unrestricted sources) and the payment.
type Found = ((usize, usize, usize, usize), Payment);

/// The most nodes [`exhaustive`] looks at before it gives up.
const SEARCH_BUDGET: usize = 20_000;

/// Every combination of floating mana and permanents added to the parts in `forced` that is a payment of the plain
/// `cost` with nothing to spare; the one with the fewest sacrifices, then the fewest sources that do more than add
/// mana, then the fewest sources, within [`SEARCH_BUDGET`] nodes.
fn exhaustive(
    cost: &ManaCost,
    pool: &Floating,
    groups: &[Vec<ManaOption>],
    forced: &Forced<'_>,
    sources: &[ManaOption],
) -> Option<Payment> {
    struct Walk<'a> {
        cost: &'a ManaCost,
        sources: &'a [ManaOption],
        candidates: Vec<Candidate<'a>>,
        payment: Payment,
        best: Option<Found>,
        budget: usize,
    }
    impl Walk<'_> {
        fn visit(&mut self, from: usize) {
            if self.budget == 0 {
                return;
            }
            self.budget -= 1;
            if is_covered(self.cost, &self.payment, self.sources) {
                // Anything added to a payment that covers the cost is a part to spare.
                if spent(self.cost, &self.payment, self.sources).is_ok() {
                    let used = taps_of(&self.payment, self.sources);
                    let weight = (
                        used.iter().filter(|o| o.sacrifice).count(),
                        used.iter().filter(|o| o.extra).count(),
                        used.len(),
                        used.iter().filter(|o| o.only.is_empty()).count(),
                    );
                    if self.best.as_ref().is_none_or(|(held, _)| weight < *held) {
                        self.best = Some((weight, self.payment.clone()));
                    }
                }
                return;
            }
            for at in from..self.candidates.len() {
                match &self.candidates[at] {
                    Candidate::Pool(mana, available) => {
                        let (mana, available) = (*mana, *available);
                        if self.payment.pool.iter().filter(|m| **m == mana).count() < available {
                            self.payment.pool.push(mana);
                            self.visit(at);
                            self.payment.pool.pop();
                        }
                    }
                    Candidate::Restricted(unit) => {
                        self.payment.restricted.push((*unit).clone());
                        self.visit(at + 1);
                        self.payment.restricted.pop();
                    }
                    Candidate::Permanent(options) => {
                        let options = options.to_vec();
                        for option in options {
                            self.payment.taps.push(option.source);
                            self.visit(at + 1);
                            self.payment.taps.pop();
                        }
                    }
                }
            }
        }
    }
    let free = pool.free_counts();
    let mut candidates: Vec<Candidate<'_>> = Vec::new();
    for mana in ManaType::ALL {
        let given = forced.pool.iter().filter(|m| **m == mana).count();
        let available = (free[mana.index()] as usize).saturating_sub(given);
        if available > 0 {
            candidates.push(Candidate::Pool(mana, available + given));
        }
    }
    let mut spare: Vec<&Restricted> = pool.units().iter().collect();
    for unit in forced.restricted {
        if let Some(at) = spare.iter().position(|held| *held == unit) {
            spare.remove(at);
        }
    }
    candidates.extend(spare.into_iter().map(Candidate::Restricted));
    candidates.extend(
        groups
            .iter()
            .filter(|group| {
                !group
                    .iter()
                    .any(|o| forced.taps.iter().any(|tap| tap.object == o.source.object))
            })
            .map(|group| Candidate::Permanent(group)),
    );
    let mut walk = Walk {
        cost,
        sources,
        candidates,
        payment: Payment {
            pool: forced.pool.to_vec(),
            restricted: forced.restricted.to_vec(),
            taps: forced.taps.to_vec(),
            ..Payment::default()
        },
        best: None,
        budget: SEARCH_BUDGET,
    };
    walk.visit(0);
    walk.best.map(|(_, payment)| payment)
}

/// Whether the parts of `payment` have an order that covers the plain `cost`.
fn is_covered(cost: &ManaCost, payment: &Payment, sources: &[ManaOption]) -> bool {
    let options = taps_of(payment, sources);
    schedule(
        cost,
        &supply_of(&payment.pool, &payment.restricted, &options),
    )
    .is_some()
}

#[cfg(test)]
mod tests {
    //! Goal: a source that costs mana is paid for from the mana made before it, never from its own, in an order the search finds; the plan takes the fewest sources and leaves nothing to spare; floating mana with a restriction pays the cost of the payment only.
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;
    use crate::mana::ManaPool;
    use crate::spend::Spend;

    fn mana(letters: &str) -> Vec<ManaType> {
        letters
            .chars()
            .map(|c| match c {
                'W' => ManaType::White,
                'U' => ManaType::Blue,
                'B' => ManaType::Black,
                'R' => ManaType::Red,
                'G' => ManaType::Green,
                _ => ManaType::Colorless,
            })
            .collect()
    }

    fn cost(text: &str) -> ManaCost {
        ManaCost::parse(text).unwrap()
    }

    /// A source of `object` that adds `made` and costs `paid` (`""` for none).
    fn source(object: u64, made: &str, paid: &str) -> ManaOption {
        let types = mana(made);
        let run = types.iter().take_while(|m| **m == types[0]).count();
        let mut option = ManaOption::new(
            ManaSource {
                object: ObjectId(object),
                ability: 0,
            },
            types[0],
        );
        option.count = u32::try_from(run).unwrap();
        option.also = types[run..].to_vec();
        option.cost = cost(paid);
        option
    }

    fn groups(sources: &[ManaOption]) -> Vec<Vec<ManaOption>> {
        sources.iter().map(|o| vec![o.clone()]).collect()
    }

    fn planned(spell: &str, floating: &str, sources: &[ManaOption]) -> Option<Payment> {
        let mut pool = ManaPool::default();
        for mana in mana(floating) {
            pool.add(mana);
        }
        auto(
            &cost(spell),
            &pool.floating(&Spend::other()),
            &groups(sources),
            &Forced::default(),
        )
    }

    fn objects(payment: &Payment) -> Vec<u64> {
        let mut found: Vec<u64> = payment.taps.iter().map(|t| t.object.0).collect();
        found.sort_unstable();
        found
    }

    #[rstest]
    #[case::a_signet_turns_one_land_into_two_colours("{W}{U}", "", &[source(0, "G", ""), source(1, "WU", "{1}")], Some(vec![0, 1]))]
    #[case::the_signet_alone_has_nothing_to_pay_with("{W}{U}", "", &[source(1, "WU", "{1}")], None)]
    #[case::floating_mana_pays_the_cost_of_the_source("{W}{U}", "G", &[source(1, "WU", "{1}")], Some(vec![1]))]
    #[case::a_signet_is_not_used_when_a_land_pays("{1}", "", &[source(0, "G", ""), source(1, "WU", "{1}")], Some(vec![0]))]
    #[case::two_signets_cannot_pay_for_each_other("{W}{U}{B}{R}", "", &[source(1, "WU", "{1}"), source(2, "BR", "{1}")], None)]
    #[case::one_land_cannot_start_two_signets("{W}{U}{B}{R}", "", &[source(0, "G", ""), source(1, "WU", "{1}"), source(2, "BR", "{1}")], None)]
    #[case::the_mana_of_one_signet_pays_the_next("{B}{R}{W}", "", &[source(0, "G", ""), source(1, "WU", "{1}"), source(2, "BR", "{1}")], Some(vec![0, 1, 2]))]
    #[case::a_prism_makes_the_colour_needed("{R}", "", &[source(0, "G", ""), source(1, "R", "{1}")], Some(vec![0, 1]))]
    #[case::a_filter_land_takes_one_of_its_colours("{U}{U}", "", &[source(0, "W", ""), source(1, "UU", "{W/U}")], Some(vec![0, 1]))]
    #[case::a_filter_land_cannot_use_a_colour_it_does_not_name("{U}{U}", "", &[source(0, "G", ""), source(1, "UU", "{W/U}")], None)]
    #[case::a_big_cost_for_a_quarry("{W}{U}{B}{R}{G}", "", &[source(0, "C", ""), source(1, "C", ""), source(2, "C", ""), source(3, "C", ""), source(4, "C", ""), source(5, "WUBRG", "{5}")], Some(vec![0, 1, 2, 3, 4, 5]))]
    fn the_sources_are_chosen_so_that_each_cost_is_paid_before_its_source_is_used(
        #[case] spell: &str,
        #[case] floating: &str,
        #[case] sources: &[ManaOption],
        #[case] expected: Option<Vec<u64>>,
    ) {
        let payment = planned(spell, floating, sources);
        assert_eq!(payment.as_ref().map(objects), expected, "{payment:?}");
        if let Some(payment) = payment {
            let spent = spent(&cost(spell), &payment, sources).unwrap();
            assert_eq!(spent.0.len(), cost(spell).mana_value() as usize);
        }
    }
}
