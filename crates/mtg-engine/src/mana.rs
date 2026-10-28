//! Mana pool and payment: validating a chosen payment and computing an automatic one.
//!
//! A cost with hybrid or Phyrexian symbols is paid by first choosing a way to pay each such symbol
//! (CR 601.2b), which leaves a plain cost to pay with mana and some life. [`ManaCost::resolutions`] lists
//! those choices, and every function here tries them and defers the mana to the plain-cost code, so a plain cost
//! takes exactly the path it always did.
//!
//! Mana that may be spent only on some things (`spend.rs`) is settled before the plain-cost code runs: for the
//! [`Spend`] a payment is for, the restricted mana and the sources that allow it join the unrestricted ones and the
//! rest is left out, so every function agrees on what can be spent. The mana a payment names is then told apart again,
//! restricted mana first, so as little of it as possible is wasted when the pool empties.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use mtg_cards::spend::{ManaLasts, SpendOn};
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType, PHYREXIAN_LIFE};
use serde::{Deserialize, Serialize};

use crate::error::PaymentError;
use crate::help::{self, Convoked, Help, Used};
use crate::mana_cost::{self, Forced};
use crate::mana_plan::{Needs, complete_with, covers_cost, plan, rest_after, spent_of};
use crate::mana_repeat;
use crate::mana_takes::{assignment, solving, sources_of, without_taken};
use crate::spend::{Restricted, Spend, Spent, Step};

/// Mana currently floating for a player.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ManaPool {
    counts: [u32; 6],
    /// The mana that may be spent only on some things, one entry for each mana; a public fact, like the pool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Restricted>>", optional))]
    // Boxed so a decision that holds a pool stays small.
    #[allow(clippy::box_collection)]
    restricted: Option<Box<Vec<Restricted>>>,
    /// Of `counts`, the mana that outlasts the step; a public fact, like the pool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Kept>", optional))]
    kept: Option<Box<Kept>>,
}

/// The mana of a pool without a restriction that stays past the end of the step, by type in the order of `ManaPool.counts`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Kept {
    /// Mana that is lost as the combat phase ends.
    pub combat_end: [u32; 6],
    /// Mana that is lost as the turn ends.
    pub turn_end: [u32; 6],
}

impl Kept {
    fn is_empty(&self) -> bool {
        self.combat_end
            .iter()
            .chain(&self.turn_end)
            .all(|held| *held == 0)
    }
}

impl ManaPool {
    /// Amount of `mana` in the pool that may be spent on anything.
    pub const fn get(&self, mana: ManaType) -> u32 {
        self.counts[mana.index()]
    }

    /// Total mana in the pool, restricted mana included.
    pub fn total(&self) -> u32 {
        self.counts
            .iter()
            .sum::<u32>()
            .saturating_add(u32::try_from(self.restricted().len()).unwrap_or(u32::MAX))
    }

    /// Whether the pool holds nothing.
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// The mana that may be spent only on some things.
    pub fn restricted(&self) -> &[Restricted] {
        self.restricted.as_deref().map_or(&[], Vec::as_slice)
    }

    pub(crate) fn add(&mut self, mana: ManaType) {
        self.counts[mana.index()] += 1;
    }

    /// The mana without a restriction that outlasts the step, if any.
    pub fn kept(&self) -> Option<&Kept> {
        self.kept.as_deref()
    }

    /// Adds one `mana` that may be spent only on `only` and stays as long as `lasts` says; with nothing listed it may be spent on anything.
    pub(crate) fn add_restricted(&mut self, mana: ManaType, only: Vec<SpendOn>, lasts: ManaLasts) {
        if !only.is_empty() {
            self.restricted
                .get_or_insert_with(Box::default)
                .push(Restricted { mana, only, lasts });
            return;
        }
        self.add(mana);
        let kept = self.kept.get_or_insert_with(Box::default);
        match lasts {
            ManaLasts::Step => {}
            ManaLasts::Combat => kept.combat_end[mana.index()] += 1,
            ManaLasts::Turn => kept.turn_end[mana.index()] += 1,
        }
        self.tidy_kept();
    }

    /// Drops the record of kept mana once it holds none, so equal pools are equal.
    fn tidy_kept(&mut self) {
        if self.kept.as_deref().is_some_and(Kept::is_empty) {
            self.kept = None;
        }
    }

    /// Removes one `mana` that may be spent on anything, the mana that goes soonest first; false (and no change) if there is none.
    pub(crate) fn spend(&mut self, mana: ManaType) -> bool {
        let at = mana.index();
        let Some(left) = self.counts[at].checked_sub(1) else {
            return false;
        };
        self.counts[at] = left;
        if let Some(kept) = self.kept.as_mut() {
            if kept.combat_end[at] + kept.turn_end[at] > left {
                if kept.combat_end[at] > 0 {
                    kept.combat_end[at] -= 1;
                } else {
                    kept.turn_end[at] -= 1;
                }
            }
            self.tidy_kept();
        }
        true
    }

    /// Removes one mana equal to `unit`; false (and no change) if there is none.
    pub(crate) fn spend_restricted(&mut self, unit: &Restricted) -> bool {
        let held = self.restricted.as_mut();
        match held.and_then(|held| {
            let at = held.iter().position(|h| h == unit)?;
            held.remove(at);
            Some(held.is_empty())
        }) {
            Some(emptied) => {
                if emptied {
                    self.restricted = None;
                }
                true
            }
            None => false,
        }
    }

    /// Empties the pool of the mana that lasts no longer than `through`: at the end of a step the mana of the step, at
    /// the end of a phase more, at the end of the turn all.
    pub(crate) fn empty(&mut self, through: ManaLasts) {
        let kept = self.kept.take();
        let held = |lasts: ManaLasts, at: usize| match (&kept, lasts) {
            (Some(kept), ManaLasts::Combat) if through < ManaLasts::Combat => kept.combat_end[at],
            (Some(kept), ManaLasts::Turn) if through < ManaLasts::Turn => kept.turn_end[at],
            _ => 0,
        };
        let mut left = Kept::default();
        for at in 0..6 {
            left.combat_end[at] = held(ManaLasts::Combat, at);
            left.turn_end[at] = held(ManaLasts::Turn, at);
            self.counts[at] = left.combat_end[at] + left.turn_end[at];
        }
        self.kept = Some(Box::new(left));
        self.tidy_kept();
        if let Some(units) = self.restricted.as_mut() {
            units.retain(|unit| unit.lasts > through);
            if units.is_empty() {
                self.restricted = None;
            }
        }
    }

    /// The shortest lifetime that takes out of the pool all that `empty(through)` would, so a log names only the
    /// lifetimes that went.
    pub fn least_through(&self, through: ManaLasts) -> ManaLasts {
        let all = self.emptying(through);
        [ManaLasts::Step, ManaLasts::Combat]
            .into_iter()
            .find(|shorter| *shorter < through && self.emptying(*shorter) == all)
            .unwrap_or(through)
    }

    /// How much mana `empty(through)` would take out of the pool.
    pub fn emptying(&self, through: ManaLasts) -> u32 {
        let mut left = self.clone();
        left.empty(through);
        self.total().saturating_sub(left.total())
    }

    /// What `spend` may use of the pool: the mana without a restriction and the restricted mana it allows.
    pub(crate) fn floating(&self, spend: &Spend) -> Floating {
        let mut floating = Floating {
            counts: self.counts,
            lean: [0; 6],
            units: Vec::new(),
        };
        for unit in self.restricted().iter().filter(|u| spend.allows(&u.only)) {
            floating.counts[unit.mana.index()] += 1;
            floating.lean[unit.mana.index()] += 1;
            floating.units.push(unit.clone());
        }
        floating
    }
}

/// The mana a payment for some [`Spend`] may use: unrestricted and allowed restricted mana alike, by type, and how much
/// of each type is restricted.
#[derive(Debug, Clone, Default)]
pub(crate) struct Floating {
    counts: [u32; 6],
    lean: [u32; 6],
    /// The restricted mana in `counts`, each with what it may be spent on.
    units: Vec<Restricted>,
}

impl Floating {
    /// The floating mana without a restriction, by type.
    pub(crate) fn free_counts(&self) -> [u32; 6] {
        let mut free = self.counts;
        for (held, lean) in free.iter_mut().zip(self.lean) {
            *held -= lean;
        }
        free
    }

    /// The restricted mana in the pool that the payment may spend.
    pub(crate) fn units(&self) -> &[Restricted] {
        &self.units
    }

    const fn get(&self, mana: ManaType) -> u32 {
        self.counts[mana.index()]
    }

    fn total(&self) -> u32 {
        self.counts.iter().sum()
    }

    fn spend(&mut self, mana: ManaType) -> bool {
        match self.counts[mana.index()].checked_sub(1) {
            Some(left) => {
                self.counts[mana.index()] = left;
                true
            }
            None => false,
        }
    }

    /// The types in the order generic mana is taken from the pool: the types that hold restricted mana first.
    fn order(&self) -> [ManaType; 6] {
        let mut order = ManaType::ALL;
        order.sort_by_key(|mana| Reverse(self.lean[mana.index()]));
        order
    }
}

/// Identifies one mana ability: the permanent and the index into its abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ManaSource {
    /// The permanent with the ability.
    pub object: ObjectId,
    /// Index into the permanent's ability list.
    pub ability: usize,
}

/// A legal mana ability and what it produces.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
// Each flag is a separate fact of one ability, and each is left out of the wire form while false.
#[allow(clippy::struct_excessive_bools)]
pub struct ManaOption {
    /// The ability.
    pub source: ManaSource,
    /// Mana added when it is activated.
    pub produces: ManaType,
    /// How many mana of that type it adds (Sol Ring adds 2); left out means one.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
    pub count: u32,
    /// Activating it does more than add mana (a painland deals damage, an egg draws); left out means it does not.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub extra: bool,
    /// Activating it does not tap its source, so a tapped permanent can still use it (a Devotee, a Lotus Petal); left out means it taps.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub tapless: bool,
    /// Activating it sacrifices its source (a Treasure); left out means it does not.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub sacrifice: bool,
    /// What the mana it adds may be spent on (Mishra's Workshop: artifact spells); left out means anything.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<SpendOn>>", optional))]
    pub only: Vec<SpendOn>,
    /// More mana of other types the same activation adds after `count` of `produces` (a bounceland's second mana); left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ManaType>>", optional))]
    pub also: Vec<ManaType>,
    /// Life the activation pays; left out means none.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
    pub life: u32,
    /// Energy the activation pays; left out means none.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
    pub energy: u32,
    /// The energy the payer holds that a payment may spend, which every source that pays energy draws on together; left out means none.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
    pub energy_held: u32,
    /// Mana the activation costs (a Signet's `{1}`), paid from the pool when it is used and so only inside a payment; left out means none.
    #[serde(default, skip_serializing_if = "no_symbols")]
    #[cfg_attr(feature = "ts", ts(as = "Option<ManaCost>", optional))]
    pub cost: ManaCost,
    /// Permanents the activation taps or sacrifices besides its source, and the ones it may take (Springleaf Drum, Skirk Prospector); paid inside a payment only, the engine picks (`mana_takes.rs`); left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Take>>", optional))]
    pub takes: Vec<Take>,
    /// A payment may use it more than once: it neither taps nor sacrifices its source and costs only what it takes (Cadaverous Bloom, Skirk Prospector), so every use takes others; left out means a payment uses it once.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub repeat: bool,
}

/// Permanents or cards a mana ability's cost takes besides its source: "Tap an untapped creature you control", "Sacrifice
/// a Goblin", "Exile a card from your hand".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Take {
    /// The permanents are sacrificed; left out means they are tapped or, with `exile`, exiled.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub sacrifice: bool,
    /// The objects are cards in the payer's hand that are exiled (Cadaverous Bloom); left out means permanents.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub exile: bool,
    /// How many it takes.
    pub count: u32,
    /// What it may take, as the cost allows it when it is offered.
    pub from: Vec<ObjectId>,
}

fn no_symbols(cost: &ManaCost) -> bool {
    cost.symbols().is_empty()
}

const fn one() -> u32 {
    1
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_one(count: &u32) -> bool {
    *count == 1
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(count: &u32) -> bool {
    *count == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

impl ManaOption {
    /// An ability that taps its source for one mana of `produces`.
    pub const fn new(source: ManaSource, produces: ManaType) -> Self {
        ManaOption {
            source,
            produces,
            count: 1,
            extra: false,
            sacrifice: false,
            tapless: false,
            only: Vec::new(),
            also: Vec::new(),
            life: 0,
            energy: 0,
            energy_held: 0,
            cost: ManaCost::ZERO,
            takes: Vec::new(),
            repeat: false,
        }
    }

    /// Whether it adds one mana, keeps its source and does nothing else, so a permanent is one mana for one tap.
    pub fn is_simple(&self) -> bool {
        self.count == 1
            && !self.sacrifice
            && !self.extra
            && self.also.is_empty()
            && self.life == 0
            && self.takes.is_empty()
            && self.energy == 0
            && !self.is_costly()
    }

    /// Whether using it costs mana, which comes from other sources of the same payment.
    pub fn is_costly(&self) -> bool {
        !self.cost.symbols().is_empty()
    }

    /// All the mana one activation adds, in order: `count` of `produces`, then `also`.
    pub fn made(&self) -> Vec<ManaType> {
        let mut made = vec![self.produces; self.count as usize];
        made.extend_from_slice(&self.also);
        made
    }
}

/// A complete, explicit payment: floating mana to spend, sources to tap and life to pay.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Payment {
    /// Floating mana to spend (a multiset).
    pub pool: Vec<ManaType>,
    /// Restricted mana to spend, named with what it may be spent on (a multiset). Left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Restricted>>", optional))]
    pub restricted: Vec<Restricted>,
    /// Mana abilities to activate for the rest.
    pub taps: Vec<ManaSource>,
    /// Life paid in place of Phyrexian symbols: 2 for each, within the life total (CR 119.4). Left out means none.
    #[serde(default)]
    pub life: u32,
    /// Creatures tapped for convoke, each with the mana it pays for (CR 702.51). Left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Convoked>>", optional))]
    pub convoke: Vec<Convoked>,
    /// Artifacts tapped for improvise, each paying {1} (CR 702.126). Left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub improvise: Vec<ObjectId>,
    /// Cards exiled from the graveyard for delve, each paying {1} (CR 702.66). Left out means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ObjectId>>", optional))]
    pub delve: Vec<ObjectId>,
}

/// The sources a payment for `spend` by a payer with `life` may tap: the ones whose mana it may spend, and the ones
/// that pay life only while the payer keeps some after every such source is used, so no payment takes the payer to 0.
fn usable(sources: &[ManaOption], spend: &Spend, life: i32) -> Vec<ManaOption> {
    let mut costs: BTreeMap<ObjectId, u32> = BTreeMap::new();
    for option in sources.iter().filter(|o| spend.allows(&o.only)) {
        let slot = costs.entry(option.source.object).or_default();
        *slot = (*slot).max(option.life);
    }
    let total: u32 = costs.values().sum();
    let affordable = i64::from(total) < i64::from(life);
    sources
        .iter()
        .filter(|o| spend.allows(&o.only) && (o.life == 0 || affordable))
        .cloned()
        .collect()
}

/// Takes `paid` energy out of what the payer holds for the sources: another part of the cost pays it.
pub(crate) fn reserve_energy(sources: &mut [ManaOption], paid: u32) {
    for option in sources {
        option.energy_held = option.energy_held.saturating_sub(paid);
    }
}

/// The most permanents that pay energy whose sets are told apart; the others are not tapped for energy.
const ENERGY_PERMANENTS: usize = 10;

/// The lists of sources a payment may draw on so that the energy they pay is there, `taps` being the abilities the
/// payment has used already (their energy is spent). It is one list while every source that pays energy can be tapped
/// together, else one for each largest set of permanents the energy covers; a permanent outside the set keeps only the
/// abilities that pay none.
fn within_energy(sources: &[ManaOption], taps: &[ManaSource]) -> Vec<Vec<ManaOption>> {
    let used = |option: &ManaOption| taps.iter().any(|tap| tap.object == option.source.object);
    let spent: u32 = taps
        .iter()
        .filter_map(|tap| sources.iter().find(|o| o.source == *tap))
        .map(|o| o.energy)
        .sum();
    let mut cost: BTreeMap<ObjectId, u32> = BTreeMap::new();
    let mut held = u32::MAX;
    for option in sources.iter().filter(|o| o.energy > 0) {
        held = held.min(option.energy_held);
        if !used(option) {
            let slot = cost.entry(option.source.object).or_default();
            *slot = (*slot).max(option.energy);
        }
    }
    let held = held.saturating_sub(spent);
    if cost.values().sum::<u32>() <= held {
        return vec![sources.to_vec()];
    }
    let permanents: Vec<(ObjectId, u32)> = cost.into_iter().take(ENERGY_PERMANENTS).collect();
    let mut lists = Vec::new();
    for set in 0u32..1 << permanents.len() {
        let chosen = |at: usize| set >> at & 1 == 1;
        let paid: u32 = (0..permanents.len())
            .filter(|at| chosen(*at))
            .map(|at| permanents[at].1)
            .sum();
        let room = held.saturating_sub(paid);
        let fits = paid <= held;
        let grows = (0..permanents.len()).any(|at| !chosen(at) && permanents[at].1 <= room);
        if !fits || grows {
            continue;
        }
        lists.push(
            sources
                .iter()
                .filter(|o| {
                    o.energy == 0
                        || used(o)
                        || permanents
                            .iter()
                            .enumerate()
                            .any(|(at, (object, _))| chosen(at) && *object == o.source.object)
                })
                .cloned()
                .collect(),
        );
    }
    lists
}

/// `payment` as the plain-cost code reads it: restricted mana is floating mana of its type.
fn flat(payment: &Payment) -> Payment {
    Payment {
        pool: payment
            .pool
            .iter()
            .copied()
            .chain(payment.restricted.iter().map(|unit| unit.mana))
            .collect(),
        restricted: Vec::new(),
        taps: payment.taps.clone(),
        life: payment.life,
        convoke: payment.convoke.clone(),
        improvise: payment.improvise.clone(),
        delve: payment.delve.clone(),
    }
}

/// Tells the mana of `types` that a payment spends from the pool apart again, on top of `kept`, the mana the payment
/// already names: restricted mana `spend` allows goes first, because it is of less use than mana without a restriction.
fn separate(types: &[ManaType], pool: &ManaPool, spend: &Spend, kept: &Payment) -> Payment {
    let mut free: Vec<&Restricted> = pool
        .restricted()
        .iter()
        .filter(|u| spend.allows(&u.only))
        .collect();
    for unit in &kept.restricted {
        if let Some(at) = free.iter().position(|held| *held == unit) {
            free.remove(at);
        }
    }
    let mut left: Vec<ManaType> = types.to_vec();
    for mana in kept
        .pool
        .iter()
        .copied()
        .chain(kept.restricted.iter().map(|unit| unit.mana))
    {
        if let Some(at) = left.iter().position(|m| *m == mana) {
            left.remove(at);
        }
    }
    let mut whole = Payment {
        pool: kept.pool.clone(),
        restricted: kept.restricted.clone(),
        taps: kept.taps.clone(),
        life: kept.life,
        convoke: kept.convoke.clone(),
        improvise: kept.improvise.clone(),
        delve: kept.delve.clone(),
    };
    for mana in left {
        match free.iter().position(|u| u.mana == mana) {
            Some(at) => whole.restricted.push(free.remove(at).clone()),
            None => whole.pool.push(mana),
        }
    }
    whole
}

/// How much mana a payment spends that is restricted: floating mana and the mana of the sources it taps.
fn restricted_used(payment: &Payment, sources: &[ManaOption]) -> usize {
    let taps = payment
        .taps
        .iter()
        .filter_map(|tap| sources.iter().find(|o| o.source == *tap))
        .filter(|o| !o.only.is_empty())
        .map(|o| o.made().len())
        .sum::<usize>();
    payment.restricted.len() + taps
}

/// Whether the payment taps a source that costs mana.
fn taps_cost_mana(payment: &Payment, sources: &[ManaOption]) -> bool {
    payment
        .taps
        .iter()
        .filter_map(|tap| sources.iter().find(|o| o.source == *tap))
        .any(ManaOption::is_costly)
}

/// The sources grouped by permanent: a permanent is tapped once, for one of its abilities.
///
/// Permanents with fewer abilities come first, so a flexible permanent is the last one tapped for a fixed need.
fn by_permanent(sources: &[ManaOption]) -> Vec<Vec<ManaOption>> {
    let mut groups: BTreeMap<ObjectId, Vec<ManaOption>> = BTreeMap::new();
    for option in sources {
        groups
            .entry(option.source.object)
            .or_default()
            .push(option.clone());
    }
    let mut groups: Vec<Vec<ManaOption>> = groups.into_values().collect();
    for group in &mut groups {
        group.sort_by_key(|o| o.source);
    }
    // Permanents that add restricted mana come first: it is of less use than mana without a restriction.
    groups.sort_by_key(|g| {
        (
            g.iter().all(|o| o.only.is_empty()),
            g.len(),
            g.first().map(|o| o.source),
        )
    });
    groups
}

fn can_make(group: &[ManaOption], mana: ManaType) -> bool {
    group.iter().any(|o| o.produces == mana)
}

fn group_source(group: &[ManaOption], mana: ManaType) -> Option<ManaSource> {
    group.iter().find(|o| o.produces == mana).map(|o| o.source)
}

/// Tries to give `needs[need]` a permanent of its own, moving earlier matches along if that frees one.
fn augment(
    need: usize,
    needs: &[ManaType],
    groups: &[Vec<ManaOption>],
    held: &mut [Option<usize>],
    seen: &mut [bool],
) -> bool {
    for (g, group) in groups.iter().enumerate() {
        if seen[g] || !can_make(group, needs[need]) {
            continue;
        }
        seen[g] = true;
        let owner = held[g];
        if owner.is_none_or(|other| augment(other, needs, groups, held, seen)) {
            held[g] = Some(need);
            return true;
        }
    }
    false
}

/// Gives every need its own permanent that can make it; the result lists, per group, the mana it serves.
fn match_needs(needs: &[ManaType], groups: &[Vec<ManaOption>]) -> Option<Vec<Option<ManaType>>> {
    let mut held: Vec<Option<usize>> = vec![None; groups.len()];
    for need in 0..needs.len() {
        let mut seen = vec![false; groups.len()];
        if !augment(need, needs, groups, &mut held, &mut seen) {
            return None;
        }
    }
    Some(held.iter().map(|h| h.map(|n| needs[n])).collect())
}

/// Whether the plain `cost` can be paid exactly from the pool plus the given sources, tapping each permanent once.
fn plain_can_afford(cost: &ManaCost, pool: &Floating, sources: &[ManaOption]) -> bool {
    if !sources.iter().all(ManaOption::is_simple) {
        return planned_auto(cost, pool, sources).is_some();
    }
    let groups = by_permanent(sources);
    let mut needs = Vec::new();
    for mana in ManaType::ALL {
        let missing = cost.required(mana).saturating_sub(pool.get(mana));
        needs.extend(std::iter::repeat_n(mana, missing as usize));
    }
    let supply = u64::from(pool.total()) + groups.len() as u64;
    supply >= u64::from(cost.mana_value()) && match_needs(&needs, &groups).is_some()
}

/// The payment of the plain `cost` when some source adds several mana or is sacrificed: floating mana first, then the
/// sources [`plan`] chooses.
fn planned_auto(cost: &ManaCost, pool: &Floating, sources: &[ManaOption]) -> Option<Payment> {
    solving(
        &[],
        sources,
        &|chosen| planned_plan(cost, pool, chosen),
        &|a, b| preference(a, sources) < preference(b, sources),
    )
}

/// [`planned_auto`] before the sources are checked for what their costs take.
fn planned_plan(cost: &ManaCost, pool: &Floating, sources: &[ManaOption]) -> Option<Payment> {
    if sources.iter().any(ManaOption::is_costly) {
        return mana_cost::auto(cost, pool, &by_permanent(sources), &Forced::default());
    }
    let mut pool_left = pool.clone();
    let mut payment = Payment::default();
    let mut needs: Needs = [0; 6];
    for mana in ManaType::ALL {
        let wanted = cost.required(mana);
        let mut taken = 0;
        while taken < wanted && pool_left.spend(mana) {
            payment.pool.push(mana);
            taken += 1;
        }
        needs[mana.index()] = wanted - taken;
    }
    let mut generic = cost.generic();
    for mana in pool.order() {
        while generic > 0 && pool_left.spend(mana) {
            payment.pool.push(mana);
            generic -= 1;
        }
    }
    payment.taps = plan(needs, generic, &by_permanent(sources), sources)?;
    // A source of several mana may cover what floating mana was spent on: a payment has nothing to spare.
    for at in (0..payment.pool.len()).rev() {
        let mut without = payment.clone();
        without.pool.remove(at);
        let parts = parts_of(pool, sources, &[], &without).ok()?;
        if covers_cost(cost, &parts) {
            payment = without;
        }
    }
    Some(payment)
}

/// Picks a valid payment of the plain `cost` if one exists: floating mana first, then sources, keeping scarce colors untapped.
fn plain_auto(cost: &ManaCost, pool: &Floating, sources: &[ManaOption]) -> Option<Payment> {
    if !sources.iter().all(ManaOption::is_simple) {
        return planned_auto(cost, pool, sources);
    }
    let mut pool_left = pool.clone();
    let mut payment = Payment::default();
    let mut take_from_pool = |mana: ManaType, wanted: u32, payment: &mut Payment| -> u32 {
        let mut taken = 0;
        while taken < wanted && pool_left.spend(mana) {
            payment.pool.push(mana);
            taken += 1;
        }
        taken
    };

    let mut needs = Vec::new();
    for mana in ManaType::ALL {
        let need = cost.required(mana);
        let missing = need - take_from_pool(mana, need, &mut payment);
        needs.extend(std::iter::repeat_n(mana, missing as usize));
    }
    let groups = by_permanent(sources);
    let mut free = Vec::new();
    for (g, mana) in match_needs(&needs, &groups)?.into_iter().enumerate() {
        match mana {
            Some(mana) => payment.taps.push(group_source(&groups[g], mana)?),
            None => free.push(g),
        }
    }

    let mut generic = cost.generic();
    for mana in pool.order() {
        generic -= take_from_pool(mana, generic, &mut payment);
    }
    for _ in 0..generic {
        let plentiful = ManaType::ALL
            .into_iter()
            .rev()
            .max_by_key(|t| free.iter().filter(|g| can_make(&groups[**g], *t)).count())?;
        let at = free.iter().position(|g| can_make(&groups[*g], plentiful))?;
        let g = free.remove(at);
        payment.taps.push(group_source(&groups[g], plentiful)?);
    }
    Some(payment)
}

/// What each part of a payment makes: one entry for every floating mana it spends and one for every tap.
///
/// The floating mana must exist, and every tap must be an offered ability of a permanent that is not
/// tapped, used once: a second ability of the same permanent would tap it again.
fn parts_of(
    pool: &Floating,
    sources: &[ManaOption],
    tapped: &[ObjectId],
    payment: &Payment,
) -> Result<Vec<Vec<ManaType>>, PaymentError> {
    let mut parts: Vec<Vec<ManaType>> = Vec::new();
    let mut from_pool = pool.clone();
    for &mana in &payment.pool {
        if !from_pool.spend(mana) {
            return Err(PaymentError::PoolShortfall(mana));
        }
        parts.push(vec![mana]);
    }
    for (i, tap) in payment.taps.iter().enumerate() {
        if payment.taps[..i].contains(tap) {
            return Err(PaymentError::DuplicateSource(*tap));
        }
        if tapped.contains(&tap.object) {
            return Err(PaymentError::AlreadyTapped(tap.object));
        }
        let option = sources
            .iter()
            .find(|o| o.source == *tap)
            .ok_or(PaymentError::UnknownSource(*tap))?;
        // Only abilities that tap their source tap it: Relic of Legends taps itself or a legendary creature.
        let taps_source = |source: &ManaSource| {
            sources
                .iter()
                .find(|o| o.source == *source)
                .is_some_and(|o| !o.tapless)
        };
        if taps_source(tap)
            && payment.taps[..i]
                .iter()
                .any(|t| t.object == tap.object && taps_source(t))
        {
            return Err(PaymentError::ObjectTappedTwice(tap.object));
        }
        parts.push(option.made());
    }
    Ok(parts)
}

/// What is left to pay of the plain `cost` once the mana of `parts` is spent: the requirements not met and the mana of
/// any kind still missing. Mana that cannot all go toward the cost is wasted.
fn left_to_pay(
    cost: &ManaCost,
    sources: &[ManaOption],
    parts: &[Vec<ManaType>],
) -> Result<(Needs, u32), PaymentError> {
    if !sources.iter().all(ManaOption::is_simple) {
        return rest_after(cost, parts);
    }
    let spent: Vec<ManaType> = parts.concat();
    let mut generic = cost.generic();
    let mut needs: Needs = [0; 6];
    for mana in ManaType::ALL {
        let have = u32::try_from(spent.iter().filter(|m| **m == mana).count()).unwrap_or(u32::MAX);
        let need = cost.required(mana);
        generic = generic
            .checked_sub(have.saturating_sub(need))
            .ok_or(PaymentError::Wasted)?;
        needs[mana.index()] = need.saturating_sub(have);
    }
    Ok((needs, generic))
}

/// Finishes a partial payment of the plain `cost`: keeps every part of `partial` (its mana, `parts`) and pays the rest.
///
/// The mana chosen must fit inside the cost (mana beyond the colored requirements has to go to the generic part).
fn plain_complete(
    cost: &ManaCost,
    pool: &Floating,
    sources: &[ManaOption],
    partial: &Payment,
    parts: &[Vec<ManaType>],
) -> Result<Payment, PaymentError> {
    let (needs, generic) = left_to_pay(cost, sources, parts)?;
    let mut symbols = Vec::new();
    for mana in ManaType::ALL {
        let symbol = mana
            .color()
            .map_or(ManaSymbol::Colorless, ManaSymbol::Colored);
        symbols.extend(std::iter::repeat_n(symbol, needs[mana.index()] as usize));
    }
    let rest_cost =
        ManaCost::from_symbols(std::iter::once(ManaSymbol::Generic(generic)).chain(symbols));
    let mut rest_pool = pool.clone();
    for &mana in &partial.pool {
        rest_pool.spend(mana);
    }
    let rest_sources: Vec<ManaOption> = sources
        .iter()
        .filter(|o| !partial.taps.iter().any(|t| t.object == o.source.object))
        .cloned()
        .collect();
    let chosen_taps = sources_of(sources, &partial.taps);
    let rest = solving(
        &chosen_taps,
        &rest_sources,
        &|chosen| plain_auto(&rest_cost, &rest_pool, chosen),
        &|a, b| preference(a, sources) < preference(b, sources),
    )
    .ok_or(PaymentError::CannotComplete)?;
    let whole = Payment {
        pool: [partial.pool.as_slice(), rest.pool.as_slice()].concat(),
        restricted: Vec::new(),
        taps: [partial.taps.as_slice(), rest.taps.as_slice()].concat(),
        life: 0,
        ..Payment::default()
    };
    if sources.iter().all(ManaOption::is_simple) {
        return Ok(whole);
    }
    // What the sources add beyond the rest of the cost may leave a part of the whole to spare: look for another.
    let held = parts_of(pool, sources, &[], &whole)?;
    if spent_of(cost, &held).is_ok() {
        return Ok(whole);
    }
    let floating: Vec<ManaType> = ManaType::ALL
        .into_iter()
        .flat_map(|mana| std::iter::repeat_n(mana, rest_pool.get(mana) as usize))
        .collect();
    let more = solving(
        &chosen_taps,
        &rest_sources,
        &|chosen| {
            complete_with(cost, parts, &floating, &by_permanent(chosen)).map(|(pool, taps)| {
                Payment {
                    pool,
                    taps,
                    ..Payment::default()
                }
            })
        },
        &|a, b| preference(a, sources) < preference(b, sources),
    )
    .ok_or(PaymentError::CannotComplete)?;
    Ok(Payment {
        pool: [partial.pool.as_slice(), more.pool.as_slice()].concat(),
        restricted: Vec::new(),
        taps: [partial.taps.as_slice(), more.taps.as_slice()].concat(),
        life: 0,
        ..Payment::default()
    })
}

/// The most life a payment of `cost` may take: 2 for each Phyrexian symbol, within the life total (CR 119.4).
///
/// Paying life is a choice made in steps of 2, so an odd life total rounds down.
pub fn max_life_payment(cost: &ManaCost, life: i32) -> u32 {
    let have = u32::try_from(life).unwrap_or(0);
    cost.life_symbols()
        .saturating_mul(PHYREXIAN_LIFE)
        .min(have - have % PHYREXIAN_LIFE)
}

/// Whether a life total can pay `paid` life (CR 119.4): at least that much, and any total for no payment at all.
fn can_pay_life(life: i32, paid: u32) -> bool {
    paid == 0 || i64::from(paid) <= i64::from(life)
}

/// Checks that `paid` life is an amount the cost allows and the payer has: even, within the Phyrexian symbols, within the life total.
fn check_life(cost: &ManaCost, life: i32, paid: u32) -> Result<(), PaymentError> {
    let most = cost.life_symbols().saturating_mul(PHYREXIAN_LIFE);
    if !paid.is_multiple_of(PHYREXIAN_LIFE) || paid > most {
        return Err(PaymentError::BadLife { paid, most });
    }
    if !can_pay_life(life, paid) {
        return Err(PaymentError::NotEnoughLife { paid, have: life });
    }
    Ok(())
}

/// Whether `cost` can be paid for `spend`, with mana from the pool and the sources (each permanent tapped once) and with life.
///
/// Restricted mana counts only where `spend` allows it. `life` is the payer's life total. Paying life down to 0 counts
/// (CR 119.4 allows it); [`auto_payment`] never does.
pub fn can_afford(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
    spend: &Spend,
) -> bool {
    can_afford_with(cost, pool, sources, Help::NONE, life, spend)
}

/// [`can_afford`] when creatures may be tapped and graveyard cards exiled to pay part of the cost (`help`).
pub fn can_afford_with(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    help: Help<'_>,
    life: i32,
    spend: &Spend,
) -> bool {
    let floating = pool.floating(spend);
    let sources = usable(&mana_repeat::expand(sources, cost), spend, life);
    within_energy(&sources, &[]).iter().any(|sources| {
        cost.resolutions().iter().any(|r| {
            can_pay_life(life, r.life)
                && help::variants(&r.cost, help, &Used::default(), &[], sources)
                    .iter()
                    .any(|v| {
                        let sources = help::without_tapped(sources, &v.used, &Used::default());
                        plain_can_afford(&v.cost, &floating, &sources)
                    })
        })
    })
}

/// How much a payment is wanted, lower first: less life, then fewer creatures tapped and cards exiled to help, then fewer sacrifices, then fewer sources that do more than add mana, then fewer taps, then more restricted mana spent, then taps of the kinds the sources make most of.
///
/// Restricted mana is of less use than mana without a restriction, so spending it keeps the rest. The last part keeps a
/// scarce color untapped when a hybrid symbol can be paid with either of two.
#[allow(clippy::type_complexity)]
fn preference(
    payment: &Payment,
    sources: &[ManaOption],
) -> (
    u32,
    usize,
    usize,
    usize,
    usize,
    Reverse<usize>,
    Reverse<usize>,
) {
    let plenty = |mana: ManaType| sources.iter().filter(|o| o.produces == mana).count();
    let used: Vec<&ManaOption> = payment
        .taps
        .iter()
        .filter_map(|tap| sources.iter().find(|o| o.source == *tap))
        .collect();
    let common = used.iter().map(|o| plenty(o.produces)).sum();
    let sacrificed = used.iter().filter(|o| o.sacrifice).count();
    let extras = used.iter().filter(|o| o.extra).count();
    (
        payment.life,
        payment.convoke.len() + payment.improvise.len() + payment.delve.len(),
        sacrificed,
        extras,
        payment.taps.len(),
        Reverse(restricted_used(payment, sources)),
        Reverse(common),
    )
}

/// Replaces `best` with `payment` when it is wanted more, so among equals the first stays.
fn keep_better(best: &mut Option<Payment>, payment: Payment, sources: &[ManaOption]) {
    let wanted = preference(&payment, sources);
    if best
        .as_ref()
        .is_none_or(|b| wanted < preference(b, sources))
    {
        *best = Some(payment);
    }
}

/// Picks a valid payment for `spend` if one exists: mana before life, floating mana first, then sources, keeping scarce colors untapped.
///
/// Life is paid only when no way to pay the cost in mana exists, then as little as possible, and never down to
/// 0 or less. Among payments that take the same life, the one that taps fewest sources wins, then the one that
/// spends the most restricted mana, then the one that taps the more common kinds of mana.
pub fn auto_payment(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
    spend: &Spend,
) -> Option<Payment> {
    auto_payment_with(cost, pool, sources, Help::NONE, life, spend)
}

/// [`auto_payment`] when creatures may be tapped and graveyard cards exiled to pay part of the cost (`help`).
///
/// They are used only when mana cannot pay the cost without them, then as few as possible (cards last, so creatures
/// that are tapped anyway go first); mana and life are chosen for the rest as [`auto_payment`] does.
pub fn auto_payment_with(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    help: Help<'_>,
    life: i32,
    spend: &Spend,
) -> Option<Payment> {
    let floating = pool.floating(spend);
    let usable_sources = usable(&mana_repeat::expand(sources, cost), spend, life);
    let mut best = None;
    for sources in within_energy(&usable_sources, &[]) {
        for resolution in cost.resolutions() {
            if resolution.life > 0 && i64::from(life) <= i64::from(resolution.life) {
                continue;
            }
            for variant in help::variants(&resolution.cost, help, &Used::default(), &[], &sources) {
                let left = help::without_tapped(&sources, &variant.used, &Used::default());
                let payment = if left.iter().any(ManaOption::is_costly) {
                    // Mana with a restriction is told apart from the rest while the sources are chosen.
                    let planned = solving(
                        &[],
                        &left,
                        &|chosen| {
                            mana_cost::auto(
                                &variant.cost,
                                &floating,
                                &by_permanent(chosen),
                                &Forced::default(),
                            )
                        },
                        &|a, b| preference(a, &left) < preference(b, &left),
                    );
                    planned.map(|whole| Payment {
                        life: resolution.life,
                        convoke: variant.used.convoke.clone(),
                        improvise: variant.used.improvise.clone(),
                        delve: variant.used.delve.clone(),
                        ..whole
                    })
                } else {
                    plain_auto(&variant.cost, &floating, &left).map(|plain| {
                        let kept = Payment {
                            taps: plain.taps,
                            life: resolution.life,
                            convoke: variant.used.convoke.clone(),
                            improvise: variant.used.improvise.clone(),
                            delve: variant.used.delve.clone(),
                            ..Payment::default()
                        };
                        separate(&plain.pool, pool, spend, &kept)
                    })
                };
                if let Some(payment) = payment {
                    keep_better(&mut best, payment, &usable_sources);
                }
            }
        }
    }
    best.map(mana_repeat::fold)
}

/// Checks the floating mana a payment names against the pool: mana without a restriction must be there, and restricted
/// mana must be there and allowed by `spend`. The mana `spend` may use is what is left to read the payment against.
fn check_pool(
    pool: &ManaPool,
    sources: &[ManaOption],
    spend: &Spend,
    payment: &Payment,
) -> Result<Floating, PaymentError> {
    let mut left = pool.clone();
    for &mana in &payment.pool {
        if !left.spend(mana) {
            return Err(PaymentError::PoolShortfall(mana));
        }
    }
    for unit in &payment.restricted {
        if !spend.allows(&unit.only) {
            return Err(PaymentError::Restricted(unit.mana));
        }
        if !left.spend_restricted(unit) {
            return Err(PaymentError::PoolShortfall(unit.mana));
        }
    }
    for tap in &payment.taps {
        if let Some(option) = sources.iter().find(|o| o.source == *tap)
            && !spend.allows(&option.only)
        {
            return Err(PaymentError::Restricted(option.produces));
        }
    }
    Ok(pool.floating(spend))
}

/// The mana of `spent` told apart: what came from restricted mana (the payment's own, or what a tap adds), which is
/// spent before mana without a restriction.
fn split_spent(spent: Vec<ManaType>, payment: &Payment, sources: &[ManaOption]) -> Spent {
    let mut units: Vec<Restricted> = payment.restricted.clone();
    for tap in &payment.taps {
        if let Some(option) = sources.iter().find(|o| o.source == *tap)
            && !option.only.is_empty()
        {
            units.extend(option.made().into_iter().map(|mana| Restricted {
                mana,
                only: option.only.clone(),
                lasts: ManaLasts::Step,
            }));
        }
    }
    let mut restricted = Vec::new();
    for mana in &spent {
        if let Some(at) = units.iter().position(|unit| unit.mana == *mana) {
            restricted.push(units.remove(at));
        }
    }
    Spent {
        mana: spent,
        restricted,
        steps: Vec::new(),
    }
}

/// Validates an explicit payment for `spend` and returns the mana it spends.
///
/// The payment must cover the cost exactly: no overpaying, no missing colored mana, and its life must be
/// what some way of paying the Phyrexian symbols takes. Restricted mana is spent only where `spend` allows it.
/// `tapped` lists permanents that are already tapped, which no payment may use; `life` is the payer's life total.
pub fn check_payment(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    tapped: &[ObjectId],
    life: i32,
    spend: &Spend,
    payment: &Payment,
) -> Result<Spent, PaymentError> {
    check_payment_with(
        cost,
        pool,
        sources,
        Help::NONE,
        tapped,
        life,
        spend,
        payment,
    )
}

/// [`check_payment`] for a payment that may name creatures tapped and graveyard cards exiled (`help` lists the ones offered).
///
/// What they pay is taken out of the cost before the mana is read against it: a creature pays {1} or one mana of a
/// colour it is, a card pays {1}, none pays more than the cost has, and none is also tapped for mana. `tapped` lists the
/// permanents and cards no payment may use.
// Every argument is a different thing a payment depends on; the callers each have them all.
#[allow(clippy::too_many_arguments)]
pub fn check_payment_with(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    help: Help<'_>,
    tapped: &[ObjectId],
    life: i32,
    spend: &Spend,
    payment: &Payment,
) -> Result<Spent, PaymentError> {
    mana_repeat::forged(payment)?;
    let sources = mana_repeat::expand(sources, cost);
    let payment = mana_repeat::unfold(payment, &sources);
    check_expanded(cost, pool, &sources, help, tapped, life, spend, &payment)
        .map(mana_repeat::fold_spent)
}

/// [`check_payment_with`] over the sources `mana_repeat::expand` made, for a payment that names the copies.
#[allow(clippy::too_many_arguments)]
fn check_expanded(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    help: Help<'_>,
    tapped: &[ObjectId],
    life: i32,
    spend: &Spend,
    payment: &Payment,
) -> Result<Spent, PaymentError> {
    let floating = check_pool(pool, sources, spend, payment)?;
    let sources = usable(sources, spend, life);
    let used = Used::new(&payment.convoke, &payment.improvise, &payment.delve);
    let tapped_for_mana: Vec<ObjectId> = payment
        .taps
        .iter()
        .map(|tap| mana_repeat::real(tap.object))
        .collect();
    help::check(&used, help, tapped, &tapped_for_mana)?;
    let parts = parts_of(&floating, &sources, tapped, &flat(payment))?;
    check_life(cost, life, payment.life)?;
    let used_sources: Vec<ManaOption> = sources_of(&sources, &payment.taps)
        .into_iter()
        .cloned()
        .collect();
    let takers = without_taken(&used_sources, &|id| used.taps(id));
    if assignment(&takers.iter().collect::<Vec<_>>()).is_none() {
        return Err(PaymentError::CannotTake);
    }
    let from_sources: u32 = payment
        .taps
        .iter()
        .filter_map(|tap| sources.iter().find(|o| o.source == *tap))
        .map(|o| o.life)
        .sum();
    if !can_pay_life(life, payment.life.saturating_add(from_sources)) {
        return Err(PaymentError::NotEnoughLife {
            paid: payment.life.saturating_add(from_sources),
            have: life,
        });
    }
    let tapped_options = payment
        .taps
        .iter()
        .filter_map(|tap| sources.iter().find(|o| o.source == *tap))
        .filter(|o| o.energy > 0);
    let (paid, have) = tapped_options.fold((0u32, 0u32), |(paid, have), o| {
        (paid.saturating_add(o.energy), have.max(o.energy_held))
    });
    if paid > have {
        return Err(PaymentError::NotEnoughEnergy { paid, have });
    }
    let mut refusal = None;
    for resolution in cost.resolutions() {
        if resolution.life != payment.life {
            continue;
        }
        let plain = match help::reduce(&resolution.cost, &used) {
            Ok(plain) => plain,
            Err(error) => {
                refusal.get_or_insert(error);
                continue;
            }
        };
        let outcome = if taps_cost_mana(payment, &sources) {
            mana_cost::spent(&plain, payment, &sources)
        } else {
            spent_of(&plain, &parts).map(|made| {
                let steps = payment
                    .taps
                    .iter()
                    .map(|tap| Step {
                        source: *tap,
                        paid: Vec::new(),
                    })
                    .collect();
                (made, steps)
            })
        };
        match outcome {
            Ok((made, steps)) => {
                let mut paid = split_spent(made, payment, &sources);
                paid.steps = steps;
                return Ok(paid);
            }
            Err(error) => {
                refusal.get_or_insert(error);
            }
        }
    }
    Err(refusal.unwrap_or(PaymentError::BadLife {
        paid: payment.life,
        most: cost.life_symbols().saturating_mul(PHYREXIAN_LIFE),
    }))
}

/// Whether a completion may pay `total` life when `kept` is already chosen: it never un-pays, and it only adds life that leaves some.
fn may_pay_life(total: u32, kept: u32, life: i32) -> bool {
    let leaves_some = i64::from(life) - i64::from(total) >= 1;
    total >= kept && can_pay_life(life, total) && (total == kept || leaves_some)
}

/// Finishes a partial payment for `spend`: keeps every part of `partial` and adds the rest automatically.
///
/// The parts are checked like a full payment's, and they must fit inside the cost (mana beyond the
/// colored requirements has to go to the generic part). The life in `partial` is kept; more life is added
/// only when no mana can pay for the rest, as [`auto_payment`] does. The result passes [`check_payment`].
pub fn complete_payment(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    tapped: &[ObjectId],
    life: i32,
    spend: &Spend,
    partial: &Payment,
) -> Result<Payment, PaymentError> {
    complete_payment_with(
        cost,
        pool,
        sources,
        Help::NONE,
        tapped,
        life,
        spend,
        partial,
    )
}

/// [`complete_payment`] when creatures may be tapped and graveyard cards exiled to pay part of the cost (`help`).
///
/// The creatures and cards `partial` names are kept; more are added only when the mana left cannot pay the rest without
/// them, as [`auto_payment_with`] does.
// Every argument is a different thing a payment depends on; the callers each have them all.
#[allow(clippy::too_many_arguments)]
pub fn complete_payment_with(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    help: Help<'_>,
    tapped: &[ObjectId],
    life: i32,
    spend: &Spend,
    partial: &Payment,
) -> Result<Payment, PaymentError> {
    mana_repeat::forged(partial)?;
    let sources = mana_repeat::expand(sources, cost);
    let partial = mana_repeat::unfold(partial, &sources);
    complete_expanded(cost, pool, &sources, help, tapped, life, spend, &partial)
        .map(mana_repeat::fold)
}

/// [`complete_payment_with`] over the sources `mana_repeat::expand` made, for a payment that names the copies.
#[allow(clippy::too_many_arguments)]
fn complete_expanded(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    help: Help<'_>,
    tapped: &[ObjectId],
    life: i32,
    spend: &Spend,
    partial: &Payment,
) -> Result<Payment, PaymentError> {
    let floating = check_pool(pool, sources, spend, partial)?;
    let usable_sources = usable(sources, spend, life);
    let flat_partial = flat(partial);
    let kept = Used::new(&partial.convoke, &partial.improvise, &partial.delve);
    let occupied: Vec<ObjectId> = partial
        .taps
        .iter()
        .map(|tap| mana_repeat::real(tap.object))
        .collect();
    help::check(&kept, help, tapped, &occupied)?;
    let parts = parts_of(&floating, &usable_sources, tapped, &flat_partial)?;
    check_life(cost, life, partial.life)?;
    let lists = within_energy(&usable_sources, &partial.taps);
    let mut best = None;
    let mut refusal: Option<PaymentError> = None;
    for resolution in cost.resolutions() {
        if !may_pay_life(resolution.life, partial.life, life) {
            continue;
        }
        let left = match help::reduce(&resolution.cost, &kept) {
            Ok(left) => left,
            Err(error) => {
                refusal.get_or_insert(error);
                continue;
            }
        };
        let helped = lists.iter().flat_map(|listed| {
            help::variants(&left, help, &kept, &occupied, listed)
                .into_iter()
                .map(move |variant| (listed, variant))
        });
        for (listed, variant) in helped {
            let sources = help::without_tapped(listed, &variant.used, &kept);
            let finished = if sources.iter().any(ManaOption::is_costly) {
                let forced = Forced {
                    pool: &partial.pool,
                    restricted: &partial.restricted,
                    taps: &partial.taps,
                };
                let chosen_taps = sources_of(&sources, &partial.taps);
                solving(
                    &chosen_taps,
                    &sources,
                    &|chosen| {
                        mana_cost::auto(&variant.cost, &floating, &by_permanent(chosen), &forced)
                    },
                    &|a, b| preference(a, &sources) < preference(b, &sources),
                )
                .ok_or(PaymentError::CannotComplete)
            } else {
                plain_complete(&variant.cost, &floating, &sources, &flat_partial, &parts).map(
                    |whole| {
                        let mut split = separate(&whole.pool, pool, spend, partial);
                        split.taps = whole.taps;
                        split
                    },
                )
            };
            match finished {
                Ok(mut whole_split) => {
                    let all = help::together(&kept, &variant.used);
                    whole_split.life = resolution.life;
                    whole_split.convoke = all.convoke;
                    whole_split.improvise = all.improvise;
                    whole_split.delve = all.delve;
                    keep_better(&mut best, whole_split, &usable_sources);
                }
                // Of the reasons, "could not be finished" says more than "does not fit".
                Err(error) => {
                    if refusal.is_none() || error == PaymentError::CannotComplete {
                        refusal = Some(error);
                    }
                }
            }
        }
    }
    let whole = best.ok_or_else(|| refusal.unwrap_or(PaymentError::CannotComplete))?;
    check_expanded(cost, pool, sources, help, tapped, life, spend, &whole)?;
    Ok(whole)
}

#[cfg(test)]
mod tests {
    //! Goal: payment validation, auto-payment and completion of partial payments agree with an independent symbol-by-symbol check for plain, hybrid and Phyrexian costs, tapping each permanent once and paying life only as the rules allow.
    use super::*;
    use mtg_core::color::Color;
    use proptest::prelude::*;
    use rstest::rstest;

    // The unit tests of plain costs pay for neither a spell nor an ability, where restricted mana plays no part.
    fn can_afford(cost: &ManaCost, pool: &ManaPool, sources: &[ManaOption], life: i32) -> bool {
        super::can_afford(cost, pool, sources, life, &Spend::other())
    }

    fn auto_payment(
        cost: &ManaCost,
        pool: &ManaPool,
        sources: &[ManaOption],
        life: i32,
    ) -> Option<Payment> {
        super::auto_payment(cost, pool, sources, life, &Spend::other())
    }

    fn check_payment(
        cost: &ManaCost,
        pool: &ManaPool,
        sources: &[ManaOption],
        tapped: &[ObjectId],
        life: i32,
        payment: &Payment,
    ) -> Result<Vec<ManaType>, PaymentError> {
        super::check_payment(cost, pool, sources, tapped, life, &Spend::other(), payment)
            .map(|spent| spent.mana)
    }

    fn complete_payment(
        cost: &ManaCost,
        pool: &ManaPool,
        sources: &[ManaOption],
        tapped: &[ObjectId],
        life: i32,
        partial: &Payment,
    ) -> Result<Payment, PaymentError> {
        super::complete_payment(cost, pool, sources, tapped, life, &Spend::other(), partial)
    }

    /// A life total with room to pay life.
    const LIFE: i32 = 20;

    fn cost(text: &str) -> ManaCost {
        ManaCost::parse(text).unwrap()
    }

    fn pool_of(mana: &[ManaType]) -> ManaPool {
        let mut pool = ManaPool::default();
        for m in mana {
            pool.add(*m);
        }
        pool
    }

    fn mana_of(c: char) -> ManaType {
        ManaType::ALL
            .into_iter()
            .find(|t| t.symbol() == c)
            .expect("valid mana letter")
    }

    fn mana_list(letters: &str) -> Vec<ManaType> {
        letters.chars().map(mana_of).collect()
    }

    /// One permanent per letter, each with a single ability.
    fn lands(types: &str) -> Vec<ManaOption> {
        let each: Vec<String> = types.chars().map(String::from).collect();
        permanents(&each.iter().map(String::as_str).collect::<Vec<_>>())
    }

    /// One permanent per entry; each letter of an entry is one of its mana abilities.
    fn permanents(abilities: &[&str]) -> Vec<ManaOption> {
        abilities
            .iter()
            .zip(0u64..)
            .flat_map(|(letters, object)| {
                letters.chars().enumerate().map(move |(ability, c)| {
                    ManaOption::new(
                        ManaSource {
                            object: ObjectId(object),
                            ability,
                        },
                        mana_of(c),
                    )
                })
            })
            .collect()
    }

    fn tap(object: u64, ability: usize) -> ManaSource {
        ManaSource {
            object: ObjectId(object),
            ability,
        }
    }

    fn taps(of: &[(u64, usize)]) -> Payment {
        Payment {
            taps: of.iter().map(|(o, a)| tap(*o, *a)).collect(),
            ..Payment::default()
        }
    }

    fn paying_life(life: u32, of: &[(u64, usize)]) -> Payment {
        Payment { life, ..taps(of) }
    }

    /// A permanent of six abilities is one tap, and each Phyrexian symbol it cannot pay with life needs its own white
    /// source: the refusal of {5}{W/P}{W/P}{W/P} at 1 life is right exactly when fewer than three permanents make white.
    #[rstest]
    #[case::three_white_sources_and_five_more(&["WUBRGC", "W", "W", "G", "G", "B", "U", "R"], 1, true)]
    #[case::the_many_ability_land_last(&["W", "W", "G", "G", "B", "U", "R", "WUBRGC"], 1, true)]
    #[case::only_two_make_white(&["WUBRGC", "W", "G", "G", "B", "U", "R", "C"], 1, false)]
    #[case::two_make_white(&["GUBRC", "W", "W", "G", "G", "B", "U", "R"], 1, false)]
    #[case::seven_permanents(&["WUBRGC", "W", "W", "G", "G", "B", "U"], 1, false)]
    #[case::life_pays_one_symbol(&["WUBRGC", "W", "G", "G", "B", "U", "R"], 2, true)]
    fn a_permanent_of_many_abilities_is_one_tap_of_a_cost_with_phyrexian_symbols(
        #[case] abilities: &[&str],
        #[case] life: i32,
        #[case] affordable: bool,
    ) {
        let cost = cost("{5}{W/P}{W/P}{W/P}");
        let sources = permanents(abilities);
        assert_eq!(can_afford(&cost, &pool_of(&[]), &sources, life), affordable);
    }

    /// `can_afford` and `auto_payment` for a cost over single-ability lands; the life automatic payment pays, if it pays at all.
    fn afford(cost_text: &str, lands_str: &str, floating: &str, life: i32) -> (bool, Option<u32>) {
        let cost = cost(cost_text);
        let pool = pool_of(&mana_list(floating));
        let sources = lands(lands_str);
        let affordable = can_afford(&cost, &pool, &sources, life);
        let auto = auto_payment(&cost, &pool, &sources, life);
        if let Some(payment) = &auto {
            assert_eq!(
                check_payment(&cost, &pool, &sources, &[], life, payment).map(|_| ()),
                Ok(()),
                "automatic payments are valid"
            );
        }
        (affordable, auto.map(|p| p.life))
    }

    #[rstest]
    #[case("{1}{G}", "GG", true)]
    #[case("{1}{G}", "WW", false)]
    #[case("{G}{G}", "GW", false)]
    #[case("{2}", "W", false)]
    #[case("{2}", "WU", true)]
    #[case("{C}", "G", false)]
    #[case("{C}", "C", true)]
    #[case("{1}{C}", "GC", true)]
    #[case("{1}{C}", "GG", false)]
    #[case("{W}{U}{B}{R}{G}", "WUBRG", true)]
    #[case("{W}{U}{B}{R}{G}", "WUBRR", false)]
    #[case("{0}", "", true)]
    #[case("{3}{G}{G}", "GGWUB", true)]
    #[case("{3}{G}{G}", "GWWUB", false)]
    fn affordability_table(
        #[case] cost_text: &str,
        #[case] lands_str: &str,
        #[case] affordable: bool,
    ) {
        let cost = cost(cost_text);
        let sources = lands(lands_str);
        let pool = ManaPool::default();
        assert_eq!(can_afford(&cost, &pool, &sources, LIFE), affordable);
        let auto = auto_payment(&cost, &pool, &sources, LIFE);
        assert_eq!(auto.is_some(), affordable);
        if let Some(payment) = auto {
            assert_eq!(payment.life, 0, "a plain cost never takes life");
            assert!(check_payment(&cost, &pool, &sources, &[], LIFE, &payment).is_ok());
        }
    }

    #[rstest]
    #[case::one_part("{W/U}", "W", true, Some(0))]
    #[case::the_other_part("{W/U}", "U", true, Some(0))]
    #[case::neither_part("{W/U}", "G", false, None)]
    #[case::one_of_each("{W/U}{W/U}", "WU", true, Some(0))]
    #[case::the_same_part_twice("{W/U}{W/U}", "UU", true, Some(0))]
    #[case::one_land_wrong("{W/U}{W/U}", "UG", false, None)]
    #[case::hybrids_share_a_colour("{W/U}{U/B}", "UU", true, Some(0))]
    #[case::hybrids_use_the_other_colours("{W/U}{U/B}", "WB", true, Some(0))]
    #[case::hybrids_with_nothing_in_common("{W/U}{U/B}", "WG", false, None)]
    #[case::hybrid_beside_its_colour("{W}{W/U}", "WU", true, Some(0))]
    #[case::hybrid_beside_its_colour_twice_blue("{W}{W/U}", "UU", false, None)]
    #[case::hybrid_beside_its_colour_twice_white("{W}{W/U}", "WW", true, Some(0))]
    #[case::hybrid_and_generic("{1}{R/W}", "GR", true, Some(0))]
    #[case::three_hybrids("{R/W}{R/W}{R/W}", "RWR", true, Some(0))]
    #[case::mono_hybrid_by_colour("{2/W}", "W", true, Some(0))]
    #[case::mono_hybrid_by_generic("{2/W}", "GG", true, Some(0))]
    #[case::mono_hybrid_one_land_short("{2/W}", "G", false, None)]
    #[case::mono_hybrid_beside_generic("{1}{2/W}", "GGG", true, Some(0))]
    #[case::mono_hybrid_beside_generic_short("{1}{2/W}", "GG", false, None)]
    #[case::mono_hybrid_beside_generic_by_colour("{1}{2/W}", "GW", true, Some(0))]
    #[case::two_mono_hybrids_by_colour("{2/W}{2/U}", "WU", true, Some(0))]
    #[case::two_mono_hybrids_one_by_generic("{2/W}{2/U}", "WGG", true, Some(0))]
    #[case::two_mono_hybrids_one_short("{2/W}{2/U}", "WG", false, None)]
    #[case::reaper_king_ten_green_lands("{2/W}{2/U}{2/B}{2/R}{2/G}", "GGGGGGGGGG", true, Some(0))]
    #[case::reaper_king_nine_green_lands("{2/W}{2/U}{2/B}{2/R}{2/G}", "GGGGGGGGG", true, Some(0))]
    #[case::reaper_king_eight_green_lands("{2/W}{2/U}{2/B}{2/R}{2/G}", "GGGGGGGG", false, None)]
    #[case::reaper_king_one_land_of_each("{2/W}{2/U}{2/B}{2/R}{2/G}", "WUBRG", true, Some(0))]
    #[case::phyrexian_by_colour("{W/P}", "W", true, Some(0))]
    #[case::phyrexian_by_life("{W/P}", "G", true, Some(2))]
    #[case::phyrexian_with_no_lands("{W/P}", "", true, Some(2))]
    #[case::phyrexian_life_prefers_mana("{G/P}", "GW", true, Some(0))]
    #[case::two_phyrexian_one_colour("{B/P}{B/P}", "B", true, Some(2))]
    #[case::two_phyrexian_no_colour("{B/P}{B/P}", "G", true, Some(4))]
    #[case::dismember_with_both_colours("{1}{B/P}{B/P}", "BBG", true, Some(0))]
    #[case::dismember_with_one_colour("{1}{B/P}{B/P}", "BG", true, Some(2))]
    #[case::dismember_with_no_colour("{1}{B/P}{B/P}", "G", true, Some(4))]
    #[case::dismember_needs_a_land_for_the_generic("{1}{B/P}{B/P}", "", false, None)]
    #[case::hybrid_phyrexian_by_either_colour("{G/U/P}", "U", true, Some(0))]
    #[case::hybrid_phyrexian_by_the_other("{G/U/P}", "G", true, Some(0))]
    #[case::hybrid_phyrexian_by_life("{G/U/P}", "W", true, Some(2))]
    #[case::colorless_hybrid_by_colorless("{C/W}", "C", true, Some(0))]
    #[case::colorless_hybrid_by_colour("{C/W}", "W", true, Some(0))]
    #[case::colorless_hybrid_by_nothing_else("{C/W}", "G", false, None)]
    #[case::colorless_phyrexian_by_colorless("{C/P}", "C", true, Some(0))]
    #[case::colorless_phyrexian_by_life("{C/P}", "G", true, Some(2))]
    #[case::kozilek_with_only_generic_land("{8}{C/P}{C/P}", "GGGGGGGG", true, Some(4))]
    #[case::kozilek_with_wastes("{8}{C/P}{C/P}", "GGGGGGGGCC", true, Some(0))]
    #[case::mixed_two_lands("{2}{W/P}{G/U/P}", "GG", true, Some(4))]
    #[case::mixed_three_lands("{2}{W/P}{G/U/P}", "GGG", true, Some(2))]
    #[case::mixed_all_colours("{2}{W/P}{G/U/P}", "GGWG", true, Some(0))]
    #[case::mixed_no_lands("{2}{W/P}{G/U/P}", "", false, None)]
    #[case::everything("{1}{R/W}{2/G}{B/P}{C/U}", "RGGUC", true, Some(2))]
    fn hybrid_and_phyrexian_affordability(
        #[case] cost_text: &str,
        #[case] lands_str: &str,
        #[case] affordable: bool,
        #[case] life_paid: Option<u32>,
    ) {
        assert_eq!(
            afford(cost_text, lands_str, "", LIFE),
            (affordable, life_paid)
        );
    }

    #[rstest]
    #[case::exactly_enough_life("{W/P}", "", 2, true, None)]
    #[case::one_life_short("{W/P}", "", 1, false, None)]
    #[case::no_life("{W/P}", "", 0, false, None)]
    #[case::negative_life("{W/P}", "", -3, false, None)]
    #[case::one_to_spare("{W/P}", "", 3, true, Some(2))]
    #[case::two_symbols_exactly_enough_life("{B/P}{B/P}", "", 4, true, None)]
    #[case::two_symbols_one_short("{B/P}{B/P}", "", 3, false, None)]
    #[case::two_symbols_enough_for_one("{B/P}{B/P}", "B", 2, true, None)]
    #[case::two_symbols_one_to_spare("{B/P}{B/P}", "B", 3, true, Some(2))]
    #[case::life_cannot_pay_a_generic_symbol("{1}{W/P}", "", 20, false, None)]
    #[case::mana_pays_the_generic_symbol_life_the_rest("{1}{W/P}", "G", 2, true, None)]
    fn paying_life_needs_the_life_total_and_automatic_payment_keeps_one(
        #[case] cost_text: &str,
        #[case] lands_str: &str,
        #[case] life: i32,
        #[case] affordable: bool,
        #[case] automatic_life: Option<u32>,
    ) {
        // Where life is the last resort, automatic payment gives up at a life total it would take to 0.
        assert_eq!(
            afford(cost_text, lands_str, "", life),
            (affordable, automatic_life)
        );
    }

    #[rstest]
    #[case::no_life_left(0)]
    #[case::below_zero(-5)]
    fn a_plain_cost_needs_no_life_total_to_be_paid(#[case] life: i32) {
        let plain = cost("{1}{G}");
        let pool = ManaPool::default();
        let sources = lands("GG");
        assert!(can_afford(&plain, &pool, &sources, life));
        let payment = auto_payment(&plain, &pool, &sources, life).unwrap();
        assert_eq!(
            check_payment(&plain, &pool, &sources, &[], life, &payment).map(|_| ()),
            Ok(())
        );
        let partial = taps(&[(0, 0)]);
        assert!(complete_payment(&plain, &pool, &sources, &[], life, &partial).is_ok());
        // Only paying life needs life.
        assert!(!can_afford(&cost("{W/P}"), &pool, &[], life));
    }

    #[test]
    fn automatic_payment_never_pays_down_to_zero_but_a_chosen_payment_may() {
        let cost = cost("{W/P}");
        let pool = ManaPool::default();
        let no_sources: Vec<ManaOption> = Vec::new();
        assert!(
            can_afford(&cost, &pool, &no_sources, 2),
            "CR 119.4 allows paying all your life"
        );
        assert_eq!(auto_payment(&cost, &pool, &no_sources, 2), None);
        assert_eq!(
            auto_payment(&cost, &pool, &no_sources, 3).map(|p| p.life),
            Some(2)
        );
        let by_hand = paying_life(2, &[]);
        assert_eq!(
            check_payment(&cost, &pool, &no_sources, &[], 2, &by_hand),
            Ok(vec![])
        );
        assert_eq!(
            check_payment(&cost, &pool, &no_sources, &[], 1, &by_hand),
            Err(PaymentError::NotEnoughLife { paid: 2, have: 1 })
        );
    }

    #[test]
    fn automatic_payment_uses_floating_mana_for_a_choice_before_tapping() {
        let cost = cost("{W/U}");
        let pool = pool_of(&[ManaType::Blue]);
        let sources = lands("W");
        let payment = auto_payment(&cost, &pool, &sources, LIFE).unwrap();
        assert_eq!(
            (payment.pool.as_slice(), payment.taps.len()),
            (&[ManaType::Blue][..], 0)
        );
    }

    #[rstest]
    #[case::more_plains_than_forests("{G/W}", "WWWG", ManaType::White)]
    #[case::more_forests_than_plains("{G/W}", "GGGW", ManaType::Green)]
    #[case::equal_numbers_keep_the_first_colour_listed("{G/W}", "WG", ManaType::Green)]
    #[case::the_scarce_colour_of_three("{W/U}", "UUUWB", ManaType::Blue)]
    #[case::colorless_hybrid_prefers_the_plentiful_kind("{C/G}", "CCCG", ManaType::Colorless)]
    fn a_choice_is_paid_with_the_colour_the_player_has_most_of(
        #[case] cost_text: &str,
        #[case] lands_str: &str,
        #[case] tapped_kind: ManaType,
    ) {
        let cost = cost(cost_text);
        let sources = lands(lands_str);
        let pool = ManaPool::default();
        let kind_of = |payment: &Payment| {
            let tap = payment.taps.first()?;
            sources
                .iter()
                .find(|o| o.source == *tap)
                .map(|o| o.produces)
        };
        let auto = auto_payment(&cost, &pool, &sources, LIFE).unwrap();
        assert_eq!(kind_of(&auto), Some(tapped_kind), "automatic payment");
        let done =
            complete_payment(&cost, &pool, &sources, &[], LIFE, &Payment::default()).unwrap();
        assert_eq!(kind_of(&done), Some(tapped_kind), "completion from nothing");
    }

    #[test]
    fn automatic_payment_taps_the_only_source_that_can_pay_a_choice() {
        let cost = cost("{W/U}{W/U}");
        let sources = lands("WGU");
        let payment = auto_payment(&cost, &ManaPool::default(), &sources, LIFE).unwrap();
        let mut objects: Vec<u64> = payment.taps.iter().map(|t| t.object.0).collect();
        objects.sort_unstable();
        assert_eq!(objects, [0, 2], "the Plains and the Island, not the Forest");
    }

    #[test]
    fn automatic_payment_spends_the_life_it_must_and_no_more() {
        let cost = cost("{1}{B/P}{B/P}");
        let sources = lands("BG");
        let payment = auto_payment(&cost, &ManaPool::default(), &sources, LIFE).unwrap();
        assert_eq!(payment.life, 2);
        assert_eq!(
            payment.taps.len(),
            2,
            "the Swamp for one symbol, the Forest for the generic"
        );
    }

    #[rstest]
    #[case("{1}{G}", 20, 0)]
    #[case("{W/P}", 20, 2)]
    #[case("{W/P}", 1, 0)]
    #[case("{W/P}", 2, 2)]
    #[case("{W/P}", 3, 2)]
    #[case("{W/P}", 0, 0)]
    #[case("{W/P}", -5, 0)]
    #[case("{1}{B/P}{B/P}", 20, 4)]
    #[case("{1}{B/P}{B/P}", 3, 2)]
    #[case("{1}{B/P}{B/P}", 4, 4)]
    #[case("{G/U/P}{C/P}{R/P}", 5, 4)]
    #[case("{R/W}{2/W}{C/W}", 20, 0)]
    fn the_most_life_a_payment_may_take(
        #[case] cost_text: &str,
        #[case] life: i32,
        #[case] most: u32,
    ) {
        assert_eq!(max_life_payment(&cost(cost_text), life), most);
    }

    #[rstest]
    #[case::overpay("{G}", "GG", &[], &[(0, 0), (1, 0)], "WrongTotal")]
    #[case::underpay("{1}{G}", "GG", &[], &[(0, 0)], "WrongTotal")]
    #[case::wrong_color("{G}", "WW", &[], &[(0, 0)], "MissingMana")]
    #[case::unknown_source("{G}", "G", &[], &[(7, 0)], "UnknownSource")]
    #[case::unknown_ability("{G}", "G", &[], &[(0, 3)], "UnknownSource")]
    #[case::duplicate("{G}{G}", "GG", &[], &[(0, 0), (0, 0)], "DuplicateSource")]
    #[case::already_tapped("{G}", "G", &[0], &[(0, 0)], "AlreadyTapped")]
    #[case::one_of_two_already_tapped("{G}{G}", "GGG", &[1], &[(0, 0), (1, 0)], "AlreadyTapped")]
    #[case::already_tapped_and_not_offered("{G}", "W", &[5], &[(5, 0)], "AlreadyTapped")]
    #[case::hybrid_with_neither_colour("{W/U}", "G", &[], &[(0, 0)], "MissingMana")]
    #[case::hybrid_overpaid("{W/U}", "WU", &[], &[(0, 0), (1, 0)], "WrongTotal")]
    #[case::mono_hybrid_one_wrong_land("{2/W}", "G", &[], &[(0, 0)], "MissingMana")]
    #[case::mono_hybrid_three_lands("{2/W}", "GGG", &[], &[(0, 0), (1, 0), (2, 0)], "WrongTotal")]
    #[case::phyrexian_paid_with_the_wrong_colour("{W/P}", "G", &[], &[(0, 0)], "MissingMana")]
    fn bad_payments_are_rejected(
        #[case] cost_text: &str,
        #[case] lands_str: &str,
        #[case] tapped: &[u64],
        #[case] taps_of: &[(u64, usize)],
        #[case] error: &str,
    ) {
        let cost = cost(cost_text);
        let tapped: Vec<ObjectId> = tapped.iter().map(|o| ObjectId(*o)).collect();
        let err = check_payment(
            &cost,
            &ManaPool::default(),
            &lands(lands_str),
            &tapped,
            LIFE,
            &taps(taps_of),
        )
        .expect_err("payment must be rejected");
        assert!(format!("{err:?}").starts_with(error), "{err:?}");
    }

    #[rstest]
    #[case::either_hybrid_colour("{W/U}", "WU", &[(0, 0)], 0)]
    #[case::the_other_hybrid_colour("{W/U}", "WU", &[(1, 0)], 0)]
    #[case::mono_hybrid_by_colour("{2/W}", "WGG", &[(0, 0)], 0)]
    #[case::mono_hybrid_by_generic("{2/W}", "WGG", &[(1, 0), (2, 0)], 0)]
    #[case::colorless_hybrid_by_colorless("{C/W}", "CW", &[(0, 0)], 0)]
    #[case::phyrexian_by_life("{W/P}", "", &[], 2)]
    #[case::phyrexian_by_colour("{W/P}", "W", &[(0, 0)], 0)]
    #[case::one_phyrexian_by_life_one_by_colour("{B/P}{B/P}", "B", &[(0, 0)], 2)]
    #[case::both_phyrexian_by_life("{B/P}{B/P}", "", &[], 4)]
    #[case::dismember_by_life_and_generic("{1}{B/P}{B/P}", "G", &[(0, 0)], 4)]
    #[case::hybrid_phyrexian_by_life("{G/U/P}", "W", &[], 2)]
    #[case::colorless_phyrexian_by_life("{C/P}", "G", &[], 2)]
    #[case::mixed("{2}{W/P}{G/U/P}", "GGU", &[(0, 0), (1, 0), (2, 0)], 2)]
    fn good_payments_are_accepted(
        #[case] cost_text: &str,
        #[case] lands_str: &str,
        #[case] taps_of: &[(u64, usize)],
        #[case] life: u32,
    ) {
        let cost = cost(cost_text);
        let sources = lands(lands_str);
        let payment = paying_life(life, taps_of);
        let spent = check_payment(&cost, &ManaPool::default(), &sources, &[], LIFE, &payment)
            .expect("the payment is accepted");
        assert_eq!(spent.len(), taps_of.len());
    }

    #[rstest]
    #[case::odd_amount("{W/P}", 1, LIFE, PaymentError::BadLife { paid: 1, most: 2 })]
    #[case::more_than_the_symbols("{W/P}", 4, LIFE, PaymentError::BadLife { paid: 4, most: 2 })]
    #[case::no_phyrexian_symbol("{G}", 2, LIFE, PaymentError::BadLife { paid: 2, most: 0 })]
    #[case::not_enough_life("{W/P}", 2, 1, PaymentError::NotEnoughLife { paid: 2, have: 1 })]
    #[case::life_total_below_zero("{W/P}", 2, -1, PaymentError::NotEnoughLife { paid: 2, have: -1 })]
    #[case::two_symbols_with_life_for_one("{B/P}{B/P}", 4, 3, PaymentError::NotEnoughLife { paid: 4, have: 3 })]
    fn life_that_does_not_fit_is_rejected(
        #[case] cost_text: &str,
        #[case] paid: u32,
        #[case] life: i32,
        #[case] error: PaymentError,
    ) {
        let result = check_payment(
            &cost(cost_text),
            &ManaPool::default(),
            &[],
            &[],
            life,
            &paying_life(paid, &[]),
        );
        assert_eq!(result, Err(error));
    }

    #[test]
    fn life_that_pays_for_nothing_is_a_wrong_total() {
        // 2 life is fine for {W/P}, but a tapped Plains on top is a payment of 1 mana for a cost of 0.
        let result = check_payment(
            &cost("{W/P}"),
            &ManaPool::default(),
            &lands("W"),
            &[],
            LIFE,
            &paying_life(2, &[(0, 0)]),
        );
        assert_eq!(result, Err(PaymentError::WrongTotal { needed: 0, got: 1 }));
    }

    #[test]
    fn auto_payment_uses_floating_mana_before_tapping() {
        let cost = cost("{1}{G}");
        let pool = pool_of(&[ManaType::Green, ManaType::Red]);
        let sources = lands("GG");
        let payment = auto_payment(&cost, &pool, &sources, LIFE).unwrap();
        assert_eq!(payment.taps.len(), 0);
        assert_eq!(payment.pool.len(), 2);
    }

    #[test]
    fn auto_payment_keeps_the_scarcer_color_untapped_for_generic() {
        let cost = cost("{1}");
        let sources = lands("GGGW");
        let payment = auto_payment(&cost, &ManaPool::default(), &sources, LIFE).unwrap();
        assert_eq!(payment.taps.len(), 1);
        assert_eq!(
            payment.taps[0].object,
            ObjectId(0),
            "taps a plentiful Forest"
        );
    }

    #[rstest]
    #[case::permanents_one_permanent_cannot_pay_two("{W}{G}", &["GW"], false)]
    #[case::second_permanent_covers_the_other_colour("{W}{G}", &["GW", "G"], true)]
    #[case::flexible_permanent_serves_either("{W}", &["GW"], true)]
    #[case::two_flexible_permanents_cover_a_colour_each("{W}{G}", &["GW", "GW"], true)]
    #[case::two_flexible_permanents_not_three_mana("{W}{G}{U}", &["GW", "GW"], false)]
    #[case::permanents_are_counted_not_abilities("{3}", &["GW", "G"], false)]
    #[case::each_permanent_is_one_mana("{2}", &["GW", "G"], true)]
    #[case::more_mana_than_permanents("{R}{G}{G}", &["RG", "RW"], false)]
    #[case::a_later_need_moves_an_earlier_choice("{R}{G}", &["RG", "RW"], true)]
    #[case::hybrid_over_flexible_permanents("{W/U}{W/U}", &["WG", "UG"], true)]
    #[case::hybrid_needs_a_second_permanent("{W/U}{W/U}", &["WU"], false)]
    #[case::hybrid_and_colour_share_a_permanent("{W}{W/U}", &["WU", "U"], true)]
    #[case::hybrid_and_colour_compete_for_one_permanent("{W}{W/U}", &["WU", "G"], false)]
    #[case::mono_hybrid_over_flexible_permanents("{2/W}{2/U}", &["WG", "UG"], true)]
    fn permanents_with_several_abilities_tap_once(
        #[case] cost_text: &str,
        #[case] spec: &[&str],
        #[case] affordable: bool,
    ) {
        let cost = cost(cost_text);
        let sources = permanents(spec);
        let pool = ManaPool::default();
        assert_eq!(can_afford(&cost, &pool, &sources, LIFE), affordable);
        let auto = auto_payment(&cost, &pool, &sources, LIFE);
        assert_eq!(auto.is_some(), affordable);
        if let Some(payment) = auto {
            assert_eq!(
                check_payment(&cost, &pool, &sources, &[], LIFE, &payment).map(|_| ()),
                Ok(())
            );
        }
    }

    #[rstest]
    #[case::two_abilities_two_colours("{W}{G}", &["GW"], &[(0, 0), (0, 1)])]
    #[case::two_abilities_one_colour("{G}{G}", &["GG"], &[(0, 0), (0, 1)])]
    #[case::second_ability_after_another_tap("{G}{W}{U}", &["G", "GW", "U"], &[(0, 0), (1, 0), (1, 1)])]
    #[case::interleaved_taps("{1}{1}", &["GW", "GW"], &[(0, 0), (1, 0), (0, 1)])]
    #[case::hybrid_taps("{W/U}{W/U}", &["WU"], &[(0, 0), (0, 1)])]
    fn two_abilities_of_one_permanent_are_rejected(
        #[case] cost_text: &str,
        #[case] spec: &[&str],
        #[case] taps_of: &[(u64, usize)],
    ) {
        let cost = cost(cost_text);
        let sources = permanents(spec);
        let err = check_payment(
            &cost,
            &ManaPool::default(),
            &sources,
            &[],
            LIFE,
            &taps(taps_of),
        )
        .expect_err("a permanent cannot be tapped twice");
        assert!(matches!(err, PaymentError::ObjectTappedTwice(_)), "{err:?}");
    }

    #[test]
    fn one_ability_each_of_two_permanents_is_accepted() {
        let cost = cost("{W}{G}");
        let sources = permanents(&["GW", "GW"]);
        let ok = check_payment(
            &cost,
            &ManaPool::default(),
            &sources,
            &[],
            LIFE,
            &taps(&[(0, 1), (1, 0)]),
        );
        assert_eq!(ok, Ok(vec![ManaType::White, ManaType::Green]));
    }

    #[test]
    fn spending_more_floating_mana_than_exists_is_rejected() {
        let cost = cost("{G}");
        let payment = Payment {
            pool: vec![ManaType::Green],
            ..Payment::default()
        };
        let err = check_payment(&cost, &ManaPool::default(), &[], &[], LIFE, &payment).unwrap_err();
        assert_eq!(err, PaymentError::PoolShortfall(ManaType::Green));
    }

    #[test]
    fn pool_never_goes_negative() {
        let mut pool = ManaPool::default();
        assert!(!pool.spend(ManaType::Red));
        pool.add(ManaType::Red);
        assert!(pool.spend(ManaType::Red));
        assert!(pool.is_empty());
    }

    /// Completes `partial` and checks the whole against the cost; returns the finished payment.
    fn completed(
        cost_text: &str,
        floating: &str,
        spec: &[&str],
        tapped: &[u64],
        life: i32,
        partial: &Payment,
    ) -> Result<Payment, PaymentError> {
        let cost = cost(cost_text);
        let pool = pool_of(&mana_list(floating));
        let sources = permanents(spec);
        let tapped: Vec<ObjectId> = tapped.iter().map(|o| ObjectId(*o)).collect();
        let whole = complete_payment(&cost, &pool, &sources, &tapped, life, partial)?;
        assert_eq!(
            check_payment(&cost, &pool, &sources, &tapped, life, &whole).map(|_| ()),
            Ok(()),
            "a completion is a valid payment"
        );
        assert!(
            partial.taps.iter().all(|t| whole.taps.contains(t)),
            "the chosen taps are kept"
        );
        assert!(whole.life >= partial.life, "life already chosen is kept");
        Ok(whole)
    }

    #[rstest]
    #[case::taps_the_rest("{2}{G}", "", &["G", "G", "W", "U"], &[], &[(3, 0)], 3)]
    #[case::keeps_the_scarce_colour_for_the_colour("{1}{G}", "", &["G", "W"], &[], &[(1, 0)], 2)]
    #[case::generic_choice_is_respected("{1}{G}", "", &["G", "G", "W"], &[], &[(2, 0)], 2)]
    #[case::complete_stays_as_it_is("{G}", "", &["G", "G"], &[], &[(1, 0)], 1)]
    #[case::empty_partial_pays_everything("{1}{G}", "", &["G", "W"], &[], &[], 2)]
    #[case::uses_floating_mana_too("{1}{G}", "R", &["G", "W"], &[], &[], 2)]
    #[case::flexible_permanent_serves_what_is_left("{W}{G}", "", &["GW", "G"], &[], &[(1, 0)], 2)]
    #[case::hybrid_after_one_colour("{W/U}{W/U}", "", &["W", "U", "W"], &[], &[(0, 0)], 2)]
    #[case::hybrid_leaves_the_other_choice_open("{W/U}{G}", "", &["U", "G", "W"], &[], &[(0, 0)], 2)]
    #[case::mono_hybrid_after_a_generic_land("{2/W}", "", &["G", "G", "W"], &[], &[(0, 0)], 2)]
    #[case::mono_hybrid_after_the_colour("{2/W}", "", &["W", "G", "G"], &[], &[(0, 0)], 1)]
    #[case::phyrexian_rest_by_mana("{1}{W/P}", "", &["G", "W"], &[], &[(0, 0)], 2)]
    fn completion_pays_the_rest(
        #[case] cost_text: &str,
        #[case] floating: &str,
        #[case] spec: &[&str],
        #[case] tapped: &[u64],
        #[case] partial: &[(u64, usize)],
        #[case] pieces: usize,
    ) {
        let whole = completed(cost_text, floating, spec, tapped, LIFE, &taps(partial)).unwrap();
        assert_eq!(whole.pool.len() + whole.taps.len(), pieces);
        assert_eq!(whole.life, 0, "mana was there, so no life is paid");
    }

    #[rstest]
    #[case::life_chosen_is_kept("{W/P}{G}", "", &["G"], 2, 20, 2, 1)]
    #[case::life_added_when_mana_cannot_pay("{W/P}", "", &["G"], 0, 20, 2, 0)]
    #[case::life_added_for_one_of_two("{B/P}{B/P}", "", &["B"], 0, 20, 2, 1)]
    #[case::life_added_for_both("{B/P}{B/P}", "", &["G"], 0, 20, 4, 0)]
    #[case::chosen_life_may_take_the_last_life("{W/P}", "", &["G"], 2, 2, 2, 0)]
    #[case::mana_before_extra_life("{B/P}{B/P}", "", &["B", "B"], 2, 20, 2, 1)]
    fn completion_adds_life_only_as_a_last_resort(
        #[case] cost_text: &str,
        #[case] floating: &str,
        #[case] spec: &[&str],
        #[case] kept: u32,
        #[case] life: i32,
        #[case] paid: u32,
        #[case] taps_wanted: usize,
    ) {
        let whole = completed(
            cost_text,
            floating,
            spec,
            &[],
            life,
            &paying_life(kept, &[]),
        )
        .unwrap();
        assert_eq!((whole.life, whole.taps.len()), (paid, taps_wanted));
    }

    #[rstest]
    #[case::extra_life_would_take_the_last("{W/P}", "G", 0, 2, PaymentError::CannotComplete)]
    #[case::extra_life_would_go_below_zero("{W/P}", "G", 0, 1, PaymentError::CannotComplete)]
    #[case::chosen_life_above_the_total("{W/P}", "G", 2, 1, PaymentError::NotEnoughLife { paid: 2, have: 1 })]
    #[case::chosen_life_the_cost_cannot_take("{G}", "G", 2, 20, PaymentError::BadLife { paid: 2, most: 0 })]
    #[case::chosen_odd_life("{W/P}{W/P}", "G", 1, 20, PaymentError::BadLife { paid: 1, most: 4 })]
    fn completion_refuses_life_that_does_not_fit(
        #[case] cost_text: &str,
        #[case] lands_str: &str,
        #[case] kept: u32,
        #[case] life: i32,
        #[case] error: PaymentError,
    ) {
        let spec = [lands_str];
        let result = completed(cost_text, "", &spec, &[], life, &paying_life(kept, &[]));
        assert_eq!(result, Err(error));
    }

    #[test]
    fn completion_keeps_chosen_floating_mana() {
        let partial = Payment {
            pool: vec![ManaType::Red],
            ..Payment::default()
        };
        let whole = completed("{1}{G}", "RR", &["G", "W"], &[], LIFE, &partial).unwrap();
        assert_eq!(whole.pool, vec![ManaType::Red]);
        assert_eq!(
            whole.taps,
            vec![tap(0, 0)],
            "the second piece is the Forest"
        );
    }

    #[rstest]
    #[case::wasted_colour("{G}{G}", &["G", "G", "W"], &[(2, 0)], PaymentError::Wasted)]
    #[case::wasted_extra_of_one_colour("{1}{G}", &["G", "G", "G"], &[(0, 0), (1, 0), (2, 0)], PaymentError::Wasted)]
    #[case::rest_cannot_be_paid("{1}{G}{G}", &["G", "W", "W"], &[(1, 0)], PaymentError::CannotComplete)]
    #[case::unknown("{G}", &["G"], &[(4, 0)], PaymentError::UnknownSource(tap(4, 0)))]
    #[case::duplicate("{G}{G}", &["G", "G"], &[(0, 0), (0, 0)], PaymentError::DuplicateSource(tap(0, 0)))]
    #[case::double_tap("{G}{W}", &["GW"], &[(0, 0), (0, 1)], PaymentError::ObjectTappedTwice(ObjectId(0)))]
    #[case::hybrid_wasted("{W/U}", &["G", "W"], &[(0, 0)], PaymentError::Wasted)]
    #[case::mono_hybrid_wasted_third_land("{2/W}", &["G", "G", "G"], &[(0, 0), (1, 0), (2, 0)], PaymentError::Wasted)]
    #[case::phyrexian_wasted("{W/P}", &["G"], &[(0, 0)], PaymentError::Wasted)]
    #[case::hybrid_rest_cannot_be_paid("{W/U}{W/U}{B}", &["W", "G", "G"], &[(0, 0)], PaymentError::CannotComplete)]
    fn completion_refuses_what_cannot_be_finished(
        #[case] cost_text: &str,
        #[case] spec: &[&str],
        #[case] partial: &[(u64, usize)],
        #[case] error: PaymentError,
    ) {
        assert_eq!(
            completed(cost_text, "", spec, &[], LIFE, &taps(partial)),
            Err(error)
        );
    }

    /// Relic of Legends: two abilities of one permanent pay together when one of them does not tap it.
    #[rstest]
    #[case::the_second_taps_another(&[false, true], Ok(()))]
    #[case::both_tap_it(&[false, false], Err(PaymentError::ObjectTappedTwice(ObjectId(0))))]
    fn c10_pa_a_tapless_ability_pays_beside_one_that_taps(
        #[case] tapless: &[bool],
        #[case] verdict: Result<(), PaymentError>,
    ) {
        let cost = cost("{G}{W}");
        let mut sources = permanents(&["GW"]);
        for source in &mut sources {
            source.tapless = tapless[source.source.ability];
        }
        let payment = taps(&[(0, 0), (0, 1)]);
        let checked = check_payment(&cost, &ManaPool::default(), &sources, &[], LIFE, &payment);
        assert_eq!(checked.map(|_| ()), verdict);
    }

    #[test]
    fn completion_refuses_a_tapped_permanent_and_missing_floating_mana() {
        let tapped = completed("{G}", "", &["G"], &[0], LIFE, &taps(&[(0, 0)]));
        assert_eq!(tapped, Err(PaymentError::AlreadyTapped(ObjectId(0))));
        let partial = Payment {
            pool: vec![ManaType::Red],
            ..Payment::default()
        };
        let short = completed("{R}", "", &["R"], &[], LIFE, &partial);
        assert_eq!(short, Err(PaymentError::PoolShortfall(ManaType::Red)));
    }

    // ---- an independent check: symbol by symbol, mana unit by mana unit ----

    /// Whether `spent` mana and `life` pay exactly for `symbols`, trying every way to pay each symbol in turn.
    fn pays(symbols: &[ManaSymbol], spent: &[ManaType], life: u32) -> bool {
        fn take(spent: &[ManaType], mana: ManaType) -> Option<Vec<ManaType>> {
            let at = spent.iter().position(|m| *m == mana)?;
            let mut rest = spent.to_vec();
            rest.remove(at);
            Some(rest)
        }
        let Some((first, rest)) = symbols.split_first() else {
            return spent.is_empty() && life == 0;
        };
        let one = |mana: ManaType| take(spent, mana).is_some_and(|left| pays(rest, &left, life));
        let colour = |c: Color| one(ManaType::from(c));
        let by_life = || life >= 2 && pays(rest, spent, life - 2);
        let generic = |n: u32| {
            ManaType::ALL.into_iter().any(|mana| {
                take(spent, mana).is_some_and(|left| {
                    let mut more = vec![ManaSymbol::Generic(n - 1)];
                    more.extend_from_slice(rest);
                    pays(&more, &left, life)
                })
            })
        };
        match *first {
            ManaSymbol::Generic(0) => pays(rest, spent, life),
            ManaSymbol::Generic(n) => generic(n),
            ManaSymbol::Colored(c) => colour(c),
            ManaSymbol::Colorless => one(ManaType::Colorless),
            ManaSymbol::Hybrid(a, b) => colour(a) || colour(b),
            ManaSymbol::MonoHybrid(c) => colour(c) || generic(2),
            ManaSymbol::Phyrexian(c) => colour(c) || by_life(),
            ManaSymbol::HybridPhyrexian(a, b) => colour(a) || colour(b) || by_life(),
            ManaSymbol::ColorlessHybrid(c) => colour(c) || one(ManaType::Colorless),
            ManaSymbol::ColorlessPhyrexian => one(ManaType::Colorless) || by_life(),
        }
    }

    #[test]
    fn the_independent_check_knows_the_rules() {
        let sym = |text: &str| cost(text).symbols().to_vec();
        let w = [ManaType::White];
        assert!(pays(&sym("{W/U}"), &w, 0));
        assert!(!pays(&sym("{W/U}"), &mana_list("G"), 0));
        assert!(pays(&sym("{2/W}"), &mana_list("GG"), 0));
        assert!(pays(&sym("{2/W}"), &w, 0));
        assert!(!pays(&sym("{2/W}"), &mana_list("G"), 0));
        assert!(pays(&sym("{W/P}"), &[], 2));
        assert!(!pays(&sym("{W/P}"), &[], 1));
        assert!(!pays(&sym("{W/P}"), &w, 2), "nothing is left over");
        assert!(pays(&sym("{C/W}"), &mana_list("C"), 0));
        assert!(!pays(&sym("{C/W}"), &mana_list("G"), 0));
        assert!(pays(&sym("{1}{B/P}{B/P}"), &mana_list("G"), 4));
        assert!(pays(&sym("{1}{B/P}{B/P}"), &mana_list("BG"), 2));
    }

    /// What each part of `payment` makes: one mana for each floating mana, and what each tap adds.
    fn made_parts(payment: &Payment, sources: &[ManaOption]) -> Vec<Vec<ManaType>> {
        let mut parts: Vec<Vec<ManaType>> = payment.pool.iter().map(|m| vec![*m]).collect();
        parts.extend(
            payment
                .taps
                .iter()
                .filter_map(|t| sources.iter().find(|o| o.source == *t))
                .map(ManaOption::made),
        );
        parts
    }

    fn spent_types(payment: &Payment, sources: &[ManaOption]) -> Vec<ManaType> {
        made_parts(payment, sources).concat()
    }

    /// What one way to pay a cost asks for: mana of each type, more mana of any type, and life.
    type Way = ([usize; 6], usize, u32);

    /// Every way to pay `symbols`, symbol by symbol (the independent counterpart of `ManaCost::resolutions`).
    fn ways(symbols: &[ManaSymbol]) -> Vec<Way> {
        let Some((first, rest)) = symbols.split_first() else {
            return vec![([0; 6], 0, 0)];
        };
        let colour = |c: Color| {
            let mut asks: Way = ([0; 6], 0, 0);
            asks.0[ManaType::from(c).index()] = 1;
            asks
        };
        let colourless = || {
            let mut asks: Way = ([0; 6], 0, 0);
            asks.0[ManaType::Colorless.index()] = 1;
            asks
        };
        let generic = |n: u32| ([0; 6], n as usize, 0);
        let life = || ([0; 6], 0, 2);
        let choices: Vec<Way> = match *first {
            ManaSymbol::Generic(n) => vec![generic(n)],
            ManaSymbol::Colored(c) => vec![colour(c)],
            ManaSymbol::Colorless => vec![colourless()],
            ManaSymbol::Hybrid(a, b) => vec![colour(a), colour(b)],
            ManaSymbol::MonoHybrid(c) => vec![colour(c), generic(2)],
            ManaSymbol::Phyrexian(c) => vec![colour(c), life()],
            ManaSymbol::HybridPhyrexian(a, b) => vec![colour(a), colour(b), life()],
            ManaSymbol::ColorlessHybrid(c) => vec![colour(c), colourless()],
            ManaSymbol::ColorlessPhyrexian => vec![colourless(), life()],
        };
        let mut all = Vec::new();
        for (types, any, paid) in choices {
            for (more, more_any, more_paid) in ways(rest) {
                let mut types = types;
                for (t, n) in types.iter_mut().zip(more) {
                    *t += n;
                }
                all.push((types, any + more_any, paid + more_paid));
            }
        }
        all
    }

    /// Whether the mana of `parts` covers `way`: enough of each type, and enough in all.
    fn covers_way(way: &Way, parts: &[&Vec<ManaType>]) -> bool {
        let made: Vec<ManaType> = parts.iter().flat_map(|p| p.iter().copied()).collect();
        let (types, any, _) = way;
        ManaType::ALL
            .iter()
            .all(|t| made.iter().filter(|m| *m == t).count() >= types[t.index()])
            && made.len() >= types.iter().sum::<usize>() + any
    }

    /// Whether `payment` pays for `cost` according to the independent check and the payer has the life: for one way to
    /// pay the cost, the mana it makes covers it and no part of the payment could be left out.
    /// Whether the sources of `payment` can take what their costs take, found by trying every permanent for every slot:
    /// none twice and none that another source of the payment makes its mana with.
    fn takes_hold(payment: &Payment, sources: &[ManaOption]) -> bool {
        fn go(slots: &[Vec<ObjectId>], taken: &mut Vec<ObjectId>) -> bool {
            let Some((first, rest)) = slots.split_first() else {
                return true;
            };
            first.iter().any(|id| {
                if taken.contains(id) {
                    return false;
                }
                taken.push(*id);
                let found = go(rest, taken);
                taken.pop();
                found
            })
        }
        let used: Vec<&ManaOption> = payment
            .taps
            .iter()
            .filter_map(|t| sources.iter().find(|o| o.source == *t))
            .collect();
        let mut slots: Vec<Vec<ObjectId>> = Vec::new();
        for (at, option) in used.iter().enumerate() {
            for take in &option.takes {
                let from: Vec<ObjectId> = take
                    .from
                    .iter()
                    .copied()
                    .filter(|id| {
                        used.iter()
                            .enumerate()
                            .all(|(other, o)| other == at || o.source.object != *id)
                    })
                    .collect();
                slots.extend(std::iter::repeat_n(from, take.count as usize));
            }
        }
        go(&slots, &mut Vec::new())
    }

    /// Whether the energy the sources of `payment` pay is within what the payer holds.
    fn energy_holds(payment: &Payment, sources: &[ManaOption]) -> bool {
        let used: Vec<&ManaOption> = payment
            .taps
            .iter()
            .filter_map(|t| sources.iter().find(|o| o.source == *t))
            .filter(|o| o.energy > 0)
            .collect();
        let paid: u32 = used.iter().map(|o| o.energy).sum();
        paid <= used.iter().map(|o| o.energy_held).max().unwrap_or(0)
    }

    fn valid(cost: &ManaCost, payment: &Payment, sources: &[ManaOption], life: i32) -> bool {
        if !takes_hold(payment, sources) {
            return false;
        }
        if !energy_holds(payment, sources) {
            return false;
        }
        if payment
            .taps
            .iter()
            .filter_map(|t| sources.iter().find(|o| o.source == *t))
            .any(ManaOption::is_costly)
        {
            return valid_with_costs(cost, payment, sources, life);
        }
        let parts = made_parts(payment, sources);
        let all: Vec<&Vec<ManaType>> = parts.iter().collect();
        ways(cost.symbols()).iter().any(|way| {
            way.2 == payment.life
                && covers_way(way, &all)
                && (0..parts.len()).all(|skip| {
                    let rest: Vec<&Vec<ManaType>> = parts
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != skip)
                        .map(|(_, part)| part)
                        .collect();
                    !covers_way(way, &rest)
                })
        }) && (payment.life == 0 || i64::from(payment.life) <= i64::from(life))
    }

    /// What is left of the mana in `avail` after one way to pay `way` takes its types and then more mana of any type.
    fn left_after(way: &Way, avail: &[ManaType]) -> Vec<Vec<ManaType>> {
        let mut rest = avail.to_vec();
        for mana in ManaType::ALL {
            for _ in 0..way.0[mana.index()] {
                let Some(at) = rest.iter().position(|m| *m == mana) else {
                    return Vec::new();
                };
                rest.remove(at);
            }
        }
        let mut found = vec![rest];
        for _ in 0..way.1 {
            let mut next: Vec<Vec<ManaType>> = Vec::new();
            for left in &found {
                for mana in ManaType::ALL {
                    if let Some(at) = left.iter().position(|m| *m == mana) {
                        let mut less = left.clone();
                        less.remove(at);
                        less.sort();
                        if !next.contains(&less) {
                            next.push(less);
                        }
                    }
                }
            }
            found = next;
        }
        found
    }

    /// Whether the `taps` can be used one after another, each paying its mana cost from the mana at hand and adding its
    /// own, so that the mana left covers `way`.
    fn runs(way: &Way, avail: &[ManaType], taps: &[&ManaOption]) -> bool {
        if taps.is_empty() {
            return covers_way(way, &[&avail.to_vec()]);
        }
        (0..taps.len()).any(|first| {
            let rest: Vec<&ManaOption> = taps
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != first)
                .map(|(_, tap)| *tap)
                .collect();
            ways(taps[first].cost.symbols())
                .iter()
                .filter(|paid| paid.2 == 0)
                .any(|paid| {
                    left_after(paid, avail).into_iter().any(|mut left| {
                        left.extend(taps[first].made());
                        runs(way, &left, &rest)
                    })
                })
        })
    }

    /// [`valid`] for a payment that taps a source that costs mana: for one way to pay the cost, some order of the
    /// sources pays each one's mana cost from the mana before it and leaves mana that covers the way, and no floating
    /// mana or source could be left out.
    fn valid_with_costs(
        cost: &ManaCost,
        payment: &Payment,
        sources: &[ManaOption],
        life: i32,
    ) -> bool {
        let taps: Vec<&ManaOption> = payment
            .taps
            .iter()
            .filter_map(|t| sources.iter().find(|o| o.source == *t))
            .collect();
        ways(cost.symbols()).iter().any(|way| {
            way.2 == payment.life
                && runs(way, &payment.pool, &taps)
                && (0..payment.pool.len()).all(|skip| {
                    let mut pool = payment.pool.clone();
                    pool.remove(skip);
                    !runs(way, &pool, &taps)
                })
                && (0..taps.len()).all(|skip| {
                    let mut rest = taps.clone();
                    rest.remove(skip);
                    !runs(way, &payment.pool, &rest)
                })
        }) && (payment.life == 0 || i64::from(payment.life) <= i64::from(life))
    }

    /// The most times a permanent is used in a payment of the independent check.
    const MOST_USES: usize = 3;

    /// Every way to use some of `abilities` up to `most` times in all: each use is one ability, and the order of the uses does not matter.
    fn uses_of(abilities: &[ManaSource], most: usize) -> Vec<Vec<ManaSource>> {
        let mut found: Vec<Vec<usize>> = (0..abilities.len()).map(|at| vec![at]).collect();
        let mut from = 0;
        for _ in 1..most {
            let end = found.len();
            for at in from..end {
                let last = found[at].last().copied().unwrap_or(0);
                for next in last..abilities.len() {
                    let mut more = found[at].clone();
                    more.push(next);
                    found.push(more);
                }
            }
            from = end;
        }
        found
            .into_iter()
            .map(|uses| uses.into_iter().map(|at| abilities[at]).collect())
            .collect()
    }

    /// Every payment from some floating mana, each permanent tapped for one of its abilities or left alone (a permanent whose abilities may be used again, up to [`MOST_USES`] times in all), and every even amount of life up to `most_life`.
    fn all_payments(floating: &[ManaType], sources: &[ManaOption], most_life: u32) -> Vec<Payment> {
        let mut objects: Vec<ObjectId> = sources.iter().map(|o| o.source.object).collect();
        objects.sort_unstable();
        objects.dedup();
        let mut tap_sets: Vec<Vec<ManaSource>> = vec![Vec::new()];
        for object in objects {
            let options: Vec<&ManaOption> = sources
                .iter()
                .filter(|o| o.source.object == object)
                .collect();
            let abilities: Vec<ManaSource> = options.iter().map(|o| o.source).collect();
            let most = if options.iter().any(|o| o.repeat) {
                MOST_USES
            } else {
                1
            };
            let mut next = Vec::new();
            for taps in &tap_sets {
                next.push(taps.clone());
                for uses in uses_of(&abilities, most) {
                    let mut with = taps.clone();
                    with.extend(uses);
                    next.push(with);
                }
            }
            tap_sets = next;
        }
        let mut out = Vec::new();
        for mask in 0u32..1 << floating.len() {
            let pool: Vec<ManaType> = floating
                .iter()
                .enumerate()
                .filter(|(i, _)| (mask >> i) & 1 == 1)
                .map(|(_, m)| *m)
                .collect();
            for taps in &tap_sets {
                for life in (0..=most_life).step_by(2) {
                    out.push(Payment {
                        pool: pool.clone(),
                        restricted: Vec::new(),
                        taps: taps.clone(),
                        life,
                        ..Payment::default()
                    });
                }
            }
        }
        out
    }

    fn contains(whole: &Payment, part: &Payment) -> bool {
        let uses = |p: &Payment, tap: &ManaSource| p.taps.iter().filter(|t| *t == tap).count();
        part.taps.iter().all(|t| uses(part, t) <= uses(whole, t))
            && ManaType::ALL.into_iter().all(|m| {
                let count = |p: &Payment| p.pool.iter().filter(|x| **x == m).count();
                count(whole) >= count(part)
            })
    }

    type Scenario = (ManaCost, Vec<ManaType>, Vec<ManaOption>, i32);

    fn mana_strategy() -> impl Strategy<Value = ManaType> {
        (0usize..6).prop_map(|i| ManaType::ALL[i])
    }

    fn color_strategy() -> impl Strategy<Value = Color> {
        (0usize..5).prop_map(|i| Color::ALL[i])
    }

    fn symbol_strategy() -> impl Strategy<Value = ManaSymbol> {
        prop_oneof![
            3 => (1u32..3).prop_map(ManaSymbol::Generic),
            4 => color_strategy().prop_map(ManaSymbol::Colored),
            1 => Just(ManaSymbol::Colorless),
            3 => (color_strategy(), color_strategy()).prop_map(|(a, b)| ManaSymbol::Hybrid(a, b)),
            2 => color_strategy().prop_map(ManaSymbol::MonoHybrid),
            3 => color_strategy().prop_map(ManaSymbol::Phyrexian),
            2 => (color_strategy(), color_strategy()).prop_map(|(a, b)| ManaSymbol::HybridPhyrexian(a, b)),
            1 => color_strategy().prop_map(ManaSymbol::ColorlessHybrid),
            1 => Just(ManaSymbol::ColorlessPhyrexian),
        ]
    }

    /// Costs of up to four symbols of every kind.
    fn any_cost() -> impl Strategy<Value = ManaCost> {
        prop::collection::vec(symbol_strategy(), 0..5).prop_map(ManaCost::from_symbols)
    }

    /// Costs with generic, colored and colorless symbols only.
    fn plain_cost() -> impl Strategy<Value = ManaCost> {
        (0u32..4, prop::collection::vec(mana_strategy(), 0..4)).prop_map(|(generic, costed)| {
            ManaCost::from_symbols(
                std::iter::once(ManaSymbol::Generic(generic)).chain(
                    costed
                        .into_iter()
                        .map(|m| m.color().map_or(ManaSymbol::Colorless, ManaSymbol::Colored)),
                ),
            )
        })
    }

    /// A random scenario: a cost, floating mana, permanents with one or two mana abilities each, and the payer's life total.
    fn scenario(costs: impl Strategy<Value = ManaCost>) -> impl Strategy<Value = Scenario> {
        (
            costs,
            prop::collection::vec(mana_strategy(), 0..3),
            prop::collection::vec(prop::collection::vec(mana_strategy(), 1..3), 0..5),
            -2i32..9,
        )
            .prop_map(|(cost, floating, permanents, life)| {
                let mut sources = Vec::new();
                for (object, abilities) in (0u64..).zip(permanents) {
                    for (ability, produces) in abilities.into_iter().enumerate() {
                        sources.push(ManaOption::new(tap(object, ability), produces));
                    }
                }
                sources.truncate(7);
                (cost, floating, sources, life)
            })
    }

    /// A random scenario whose sources may add several mana or be sacrificed: up to three permanents of one or two
    /// abilities each, one floating mana at most.
    fn complex_scenario(costs: impl Strategy<Value = ManaCost>) -> impl Strategy<Value = Scenario> {
        (
            costs,
            prop::collection::vec(mana_strategy(), 0..2),
            prop::collection::vec(
                prop::collection::vec(
                    (
                        mana_strategy(),
                        1u32..4,
                        any::<bool>(),
                        any::<bool>(),
                        prop::collection::vec(mana_strategy(), 0..2),
                    ),
                    1..3,
                ),
                0..4,
            ),
            -2i32..9,
        )
            .prop_map(|(cost, floating, permanents, life)| {
                let mut sources = Vec::new();
                for (object, abilities) in (0u64..).zip(permanents) {
                    for (ability, (produces, count, sacrifice, extra, also)) in
                        abilities.into_iter().enumerate()
                    {
                        sources.push(ManaOption {
                            source: tap(object, ability),
                            produces,
                            count,
                            sacrifice,
                            extra,
                            only: Vec::new(),
                            also,
                            life: 0,
                            energy: 0,
                            energy_held: 0,
                            cost: ManaCost::ZERO,
                            ..ManaOption::new(tap(object, ability), produces)
                        });
                    }
                }
                (cost, floating, sources, life)
            })
    }

    /// Sources of several permanents of which some take others: tapping or sacrificing one to three of the permanents 0 to 5
    /// (the sources are 0 to 3, so a source may be wanted by another's cost).
    fn taking_scenario(costs: impl Strategy<Value = ManaCost>) -> impl Strategy<Value = Scenario> {
        (
            complex_scenario(costs),
            prop::collection::vec(
                (
                    any::<bool>(),
                    any::<bool>(),
                    1u32..3,
                    prop::collection::btree_set(0u64..6, 0..4),
                ),
                0..4,
            ),
            // Whether each source pays one energy, and the energy the payer holds.
            (prop::collection::vec(any::<bool>(), 0..4), 0u32..3),
        )
            .prop_map(
                |((cost, floating, mut sources, life), takes, (energy, held))| {
                    for (source, (takes, sacrifice, count, from)) in sources.iter_mut().zip(takes) {
                        if takes {
                            source.takes.push(Take {
                                sacrifice,
                                exile: false,
                                count,
                                from: from.into_iter().map(ObjectId).collect(),
                            });
                            source.extra = true;
                        }
                    }
                    // A source that takes nothing may pay one energy, which every such source draws on together.
                    for (source, pays) in sources.iter_mut().zip(energy) {
                        if pays && source.takes.is_empty() {
                            source.energy = 1;
                            source.energy_held = held;
                            source.extra = true;
                        }
                    }
                    (cost, floating, sources, life)
                },
            )
    }

    /// What a source's own activation may cost: generic, one colour, a hybrid pair or a colour with generic mana.
    fn activation_cost() -> impl Strategy<Value = ManaCost> {
        prop_oneof![
            3 => (1u32..3).prop_map(|n| ManaCost::from_symbols([ManaSymbol::Generic(n)])),
            2 => color_strategy().prop_map(|c| ManaCost::from_symbols([ManaSymbol::Colored(c)])),
            3 => (color_strategy(), color_strategy())
                .prop_map(|(a, b)| ManaCost::from_symbols([ManaSymbol::Hybrid(a, b)])),
            1 => color_strategy().prop_map(|c| {
                ManaCost::from_symbols([ManaSymbol::Generic(1), ManaSymbol::Colored(c)])
            }),
        ]
    }

    /// Sources of which some permanents may be used again (a Cadaverous Bloom): they neither tap nor sacrifice, each use
    /// exiles one or two of the 0 to 3 cards of a hand, and the cards are among the permanents 0 to 6 (so a card may be a
    /// source too).
    fn repeating_scenario(
        costs: impl Strategy<Value = ManaCost>,
    ) -> impl Strategy<Value = Scenario> {
        (
            complex_scenario(costs),
            prop::collection::vec(
                (
                    any::<bool>(),
                    1u32..3,
                    prop::collection::btree_set(0u64..7, 0..4),
                ),
                0..4,
            ),
        )
            .prop_map(|((cost, floating, mut sources, life), hands)| {
                for source in &mut sources {
                    let Some((again, count, from)) = usize::try_from(source.source.object.0)
                        .ok()
                        .and_then(|at| hands.get(at))
                    else {
                        continue;
                    };
                    if *again {
                        source.repeat = true;
                        source.tapless = true;
                        source.sacrifice = false;
                        source.extra = true;
                        source.takes = vec![Take {
                            sacrifice: false,
                            exile: true,
                            count: *count,
                            from: from.iter().copied().map(ObjectId).collect(),
                        }];
                    }
                }
                (cost, floating, sources, life)
            })
    }

    /// A random scenario whose sources may cost mana (a Signet, a filter land, a Prism): up to three permanents of one
    /// or two abilities each, at most two of them costing mana, one floating mana at most.
    fn costly_scenario(costs: impl Strategy<Value = ManaCost>) -> impl Strategy<Value = Scenario> {
        (
            costs,
            prop::collection::vec(mana_strategy(), 0..2),
            prop::collection::vec(
                prop::collection::vec(
                    (
                        mana_strategy(),
                        1u32..3,
                        any::<bool>(),
                        prop::collection::vec(mana_strategy(), 0..2),
                        prop::option::weighted(0.6, activation_cost()),
                    ),
                    1..3,
                ),
                0..4,
            ),
            -2i32..9,
        )
            .prop_map(|(cost, floating, permanents, life)| {
                let mut sources = Vec::new();
                let mut costly = 0;
                for (object, abilities) in (0u64..).zip(permanents) {
                    for (ability, (produces, count, sacrifice, also, paid)) in
                        abilities.into_iter().enumerate()
                    {
                        let paid = paid.filter(|_| {
                            costly += 1;
                            costly <= 2
                        });
                        sources.push(ManaOption {
                            source: tap(object, ability),
                            produces,
                            count,
                            sacrifice,
                            extra: false,
                            only: Vec::new(),
                            also,
                            life: 0,
                            energy: 0,
                            energy_held: 0,
                            cost: paid.unwrap_or(ManaCost::ZERO),
                            ..ManaOption::new(tap(object, ability), produces)
                        });
                    }
                }
                (cost, floating, sources, life)
            })
    }

    /// Sacrifices a payment makes.
    fn sacrifices(payment: &Payment, sources: &[ManaOption]) -> usize {
        payment
            .taps
            .iter()
            .filter_map(|t| sources.iter().find(|o| o.source == *t))
            .filter(|o| o.sacrifice)
            .count()
    }

    fn payments_of(scenario: &Scenario) -> Vec<Payment> {
        let (cost, floating, sources, _) = scenario;
        all_payments(floating, sources, cost.life_symbols() * PHYREXIAN_LIFE)
    }

    fn automatic_payment_matches_the_independent_check(
        scenario: &Scenario,
    ) -> Result<(), TestCaseError> {
        let (cost, floating, sources, life) = scenario;
        let pool = pool_of(floating);
        let payments = payments_of(scenario);
        let valid_ones: Vec<&Payment> = payments
            .iter()
            .filter(|p| valid(cost, p, sources, *life))
            .collect();
        prop_assert_eq!(
            can_afford(cost, &pool, sources, *life),
            !valid_ones.is_empty(),
            "affordable exactly when some payment is valid"
        );
        // Automatic payment never takes the last life, so it only chooses among payments that leave some.
        let allowed: Vec<&&Payment> = valid_ones
            .iter()
            .filter(|p| p.life == 0 || i64::from(*life) > i64::from(p.life))
            .collect();
        let auto = auto_payment(cost, &pool, sources, *life);
        prop_assert_eq!(auto.is_some(), !allowed.is_empty());
        if let Some(payment) = auto {
            prop_assert_eq!(
                check_payment(cost, &pool, sources, &[], *life, &payment).map(|_| ()),
                Ok(())
            );
            prop_assert!(valid(cost, &payment, sources, *life));
            let least = allowed.iter().map(|p| p.life).min();
            prop_assert_eq!(Some(payment.life), least, "mana first, then the least life");
            let fewest = allowed
                .iter()
                .filter(|p| p.life == payment.life)
                .map(|p| sacrifices(p, sources))
                .min();
            prop_assert_eq!(
                Some(sacrifices(&payment, sources)),
                fewest,
                "no more sacrifices than the life paid needs"
            );
            let extras = |p: &Payment| {
                p.taps
                    .iter()
                    .filter_map(|t| sources.iter().find(|o| o.source == *t))
                    .filter(|o| o.extra)
                    .count()
            };
            let fewest_extras = allowed
                .iter()
                .filter(|p| {
                    p.life == payment.life
                        && sacrifices(p, sources) == sacrifices(&payment, sources)
                })
                .map(|p| extras(p))
                .min();
            prop_assert_eq!(
                Some(extras(&payment)),
                fewest_extras,
                "no more sources that do more than add mana than the sacrifices allow"
            );
        }
        Ok(())
    }

    fn check_payment_matches_the_independent_check(
        scenario: &Scenario,
        pick: prop::sample::Index,
        extra_life: u32,
    ) -> Result<(), TestCaseError> {
        let (cost, floating, sources, life) = scenario;
        let pool = pool_of(floating);
        let payments = payments_of(scenario);
        let mut payment = payments[pick.index(payments.len())].clone();
        // Amounts the cost cannot take, odd ones included, are refused whatever the mana.
        payment.life += extra_life;
        let verdict = check_payment(cost, &pool, sources, &[], *life, &payment);
        prop_assert_eq!(
            verdict.is_ok(),
            valid(cost, &payment, sources, *life),
            "{:?}",
            verdict
        );
        if let Ok(spent) = verdict {
            let mut expected = spent_types(&payment, sources);
            let mut got = spent;
            expected.sort();
            got.sort();
            let costly = payment
                .taps
                .iter()
                .filter_map(|t| sources.iter().find(|o| o.source == *t))
                .any(ManaOption::is_costly);
            if !costly && made_parts(&payment, sources).iter().all(|p| p.len() == 1) {
                prop_assert_eq!(got, expected);
            } else {
                // A source that adds several mana may add more than the cost takes; the rest floats.
                let mut left = expected;
                for mana in got {
                    let at = left.iter().position(|m| *m == mana);
                    prop_assert!(at.is_some(), "{:?} was spent but not made", mana);
                    left.remove(at.unwrap_or(0));
                }
            }
        }
        Ok(())
    }

    fn completion_matches_the_independent_check(
        scenario: &Scenario,
        pick: prop::sample::Index,
    ) -> Result<(), TestCaseError> {
        let (cost, floating, sources, life) = scenario;
        let pool = pool_of(floating);
        let payments = payments_of(scenario);
        let partial = payments[pick.index(payments.len())].clone();
        let completes = |whole: &Payment| {
            valid(cost, whole, sources, *life)
                && contains(whole, &partial)
                && whole.life >= partial.life
                && (whole.life == partial.life || i64::from(*life) - i64::from(whole.life) >= 1)
        };
        let ways_to_finish: Vec<&Payment> = payments.iter().filter(|w| completes(w)).collect();
        let done = complete_payment(cost, &pool, sources, &[], *life, &partial);
        prop_assert_eq!(
            done.is_ok(),
            !ways_to_finish.is_empty(),
            "{:?} for {:?}",
            done,
            partial
        );
        if let Ok(whole) = done {
            prop_assert_eq!(
                check_payment(cost, &pool, sources, &[], *life, &whole).map(|_| ()),
                Ok(())
            );
            prop_assert!(completes(&whole));
            prop_assert_eq!(
                Some(whole.life),
                ways_to_finish.iter().map(|w| w.life).min(),
                "no more life than the cheapest finish"
            );
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        #[test]
        fn plain_costs_follow_the_independent_check(s in scenario(plain_cost())) {
            automatic_payment_matches_the_independent_check(&s)?;
        }

        #[test]
        fn any_cost_follows_the_independent_check(s in scenario(any_cost())) {
            automatic_payment_matches_the_independent_check(&s)?;
        }

        #[test]
        fn plain_payments_are_checked_like_the_independent_check(
            s in scenario(plain_cost()), pick in any::<prop::sample::Index>(), extra in 0u32..3
        ) {
            check_payment_matches_the_independent_check(&s, pick, extra)?;
        }

        #[test]
        fn any_payment_is_checked_like_the_independent_check(
            s in scenario(any_cost()), pick in any::<prop::sample::Index>(), extra in 0u32..4
        ) {
            check_payment_matches_the_independent_check(&s, pick, extra)?;
        }

        #[test]
        fn plain_completion_exists_iff_some_valid_payment_contains_the_partial(
            s in scenario(plain_cost()), pick in any::<prop::sample::Index>()
        ) {
            completion_matches_the_independent_check(&s, pick)?;
        }

        #[test]
        fn completion_exists_iff_some_valid_payment_contains_the_partial(
            s in scenario(any_cost()), pick in any::<prop::sample::Index>()
        ) {
            completion_matches_the_independent_check(&s, pick)?;
        }

        #[test]
        fn sources_of_several_mana_and_sacrifices_follow_the_independent_check(
            s in complex_scenario(any_cost())
        ) {
            automatic_payment_matches_the_independent_check(&s)?;
        }

        #[test]
        fn payments_of_such_sources_are_checked_like_the_independent_check(
            s in complex_scenario(any_cost()), pick in any::<prop::sample::Index>(), extra in 0u32..4
        ) {
            check_payment_matches_the_independent_check(&s, pick, extra)?;
        }

        #[test]
        fn completion_with_such_sources_exists_iff_some_valid_payment_contains_the_partial(
            s in complex_scenario(any_cost()), pick in any::<prop::sample::Index>()
        ) {
            completion_matches_the_independent_check(&s, pick)?;
        }

        #[test]
        fn sources_that_take_permanents_follow_the_independent_check(
            s in taking_scenario(any_cost())
        ) {
            automatic_payment_matches_the_independent_check(&s)?;
        }

        #[test]
        fn payments_with_sources_that_take_permanents_are_checked_like_the_independent_check(
            s in taking_scenario(any_cost()), pick in any::<prop::sample::Index>(), extra in 0u32..4
        ) {
            check_payment_matches_the_independent_check(&s, pick, extra)?;
        }

        #[test]
        fn completion_with_sources_that_take_permanents_exists_iff_some_valid_payment_contains_the_partial(
            s in taking_scenario(any_cost()), pick in any::<prop::sample::Index>()
        ) {
            completion_matches_the_independent_check(&s, pick)?;
        }

        #[test]
        fn sources_used_more_than_once_follow_the_independent_check(
            s in repeating_scenario(any_cost())
        ) {
            automatic_payment_matches_the_independent_check(&s)?;
        }

        #[test]
        fn sources_used_more_than_once_pay_plain_costs_like_the_independent_check(
            s in repeating_scenario(plain_cost())
        ) {
            automatic_payment_matches_the_independent_check(&s)?;
        }

        #[test]
        fn payments_that_use_a_source_more_than_once_are_checked_like_the_independent_check(
            s in repeating_scenario(any_cost()), pick in any::<prop::sample::Index>(), extra in 0u32..4
        ) {
            check_payment_matches_the_independent_check(&s, pick, extra)?;
        }

        #[test]
        fn completion_with_sources_used_more_than_once_exists_iff_some_valid_payment_contains_the_partial(
            s in repeating_scenario(any_cost()), pick in any::<prop::sample::Index>()
        ) {
            completion_matches_the_independent_check(&s, pick)?;
        }

        #[test]
        fn sources_that_cost_mana_follow_the_independent_check(s in costly_scenario(any_cost())) {
            automatic_payment_matches_the_independent_check(&s)?;
        }

        #[test]
        fn sources_that_cost_mana_pay_plain_costs_like_the_independent_check(
            s in costly_scenario(plain_cost())
        ) {
            automatic_payment_matches_the_independent_check(&s)?;
        }

        #[test]
        fn payments_with_sources_that_cost_mana_are_checked_like_the_independent_check(
            s in costly_scenario(any_cost()), pick in any::<prop::sample::Index>(), extra in 0u32..4
        ) {
            check_payment_matches_the_independent_check(&s, pick, extra)?;
        }

        #[test]
        fn completion_with_sources_that_cost_mana_exists_iff_some_valid_payment_contains_the_partial(
            s in costly_scenario(any_cost()), pick in any::<prop::sample::Index>()
        ) {
            completion_matches_the_independent_check(&s, pick)?;
        }

        #[test]
        fn a_plain_cost_is_paid_exactly_as_before_and_never_with_life(s in scenario(plain_cost())) {
            let (cost, floating, sources, life) = &s;
            let pool = pool_of(floating);
            prop_assert_eq!(
                auto_payment(cost, &pool, sources, *life),
                plain_auto(cost, &pool.floating(&Spend::other()), sources),
                "the plain path is the one that always was"
            );
            prop_assert_eq!(can_afford(cost, &pool, sources, *life), plain_can_afford(cost, &pool.floating(&Spend::other()), sources));
        }
    }
}
