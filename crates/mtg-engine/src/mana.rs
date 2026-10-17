//! Mana pool and payment: validating a chosen payment and computing an automatic one.

use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};
use serde::{Deserialize, Serialize};

use crate::error::PaymentError;

/// Mana currently floating for a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ManaPool {
    counts: [u32; 6],
}

impl ManaPool {
    /// Amount of `mana` in the pool.
    pub const fn get(&self, mana: ManaType) -> u32 {
        self.counts[mana.index()]
    }

    /// Total mana in the pool.
    pub fn total(&self) -> u32 {
        self.counts.iter().sum()
    }

    /// Whether the pool holds nothing.
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    pub(crate) fn add(&mut self, mana: ManaType) {
        self.counts[mana.index()] += 1;
    }

    /// Removes one `mana`; false (and no change) if there is none.
    pub(crate) fn spend(&mut self, mana: ManaType) -> bool {
        match self.counts[mana.index()].checked_sub(1) {
            Some(left) => {
                self.counts[mana.index()] = left;
                true
            }
            None => false,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.counts = [0; 6];
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ManaOption {
    /// The ability.
    pub source: ManaSource,
    /// Mana added when it is activated.
    pub produces: ManaType,
}

/// A complete, explicit payment: floating mana to spend and sources to tap.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Payment {
    /// Floating mana to spend (a multiset).
    pub pool: Vec<ManaType>,
    /// Mana abilities to activate for the rest.
    pub taps: Vec<ManaSource>,
}

fn supply(pool: &ManaPool, sources: &[ManaOption]) -> [u32; 6] {
    let mut counts = pool.counts;
    for option in sources {
        counts[option.produces.index()] += 1;
    }
    counts
}

/// Whether `cost` can be paid exactly from the pool plus the given sources.
pub fn can_afford(cost: &ManaCost, pool: &ManaPool, sources: &[ManaOption]) -> bool {
    let counts = supply(pool, sources);
    ManaType::ALL
        .into_iter()
        .all(|t| counts[t.index()] >= cost.required(t))
        && counts.iter().sum::<u32>() >= cost.mana_value()
}

/// Picks a valid payment if one exists: floating mana first, then sources, keeping scarce colors untapped.
pub fn auto_payment(cost: &ManaCost, pool: &ManaPool, sources: &[ManaOption]) -> Option<Payment> {
    let mut pool_left = *pool;
    let mut available: Vec<ManaOption> = sources.to_vec();
    available.sort_by_key(|o| o.source);
    let mut payment = Payment::default();

    let mut take_from_pool = |mana: ManaType, wanted: u32, payment: &mut Payment| -> u32 {
        let mut taken = 0;
        while taken < wanted && pool_left.spend(mana) {
            payment.pool.push(mana);
            taken += 1;
        }
        taken
    };
    let take_source = |mana: ManaType, available: &mut Vec<ManaOption>, payment: &mut Payment| {
        let pos = available.iter().position(|o| o.produces == mana)?;
        payment.taps.push(available.remove(pos).source);
        Some(())
    };

    for mana in ManaType::ALL {
        let need = cost.required(mana);
        let missing = need - take_from_pool(mana, need, &mut payment);
        for _ in 0..missing {
            take_source(mana, &mut available, &mut payment)?;
        }
    }

    let mut generic = cost.generic();
    for mana in ManaType::ALL {
        generic -= take_from_pool(mana, generic, &mut payment);
    }
    for _ in 0..generic {
        let plentiful = ManaType::ALL
            .into_iter()
            .rev()
            .max_by_key(|t| available.iter().filter(|o| o.produces == *t).count())?;
        take_source(plentiful, &mut available, &mut payment)?;
    }
    Some(payment)
}

/// Validates an explicit payment and returns the mana it spends.
///
/// The payment must cover the cost exactly: no overpaying, no missing colored mana.
pub fn check_payment(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    payment: &Payment,
) -> Result<Vec<ManaType>, PaymentError> {
    let mut spent: Vec<ManaType> = Vec::new();
    let mut from_pool = *pool;
    for &mana in &payment.pool {
        if !from_pool.spend(mana) {
            return Err(PaymentError::PoolShortfall(mana));
        }
        spent.push(mana);
    }
    for (i, tap) in payment.taps.iter().enumerate() {
        if payment.taps[..i].contains(tap) {
            return Err(PaymentError::DuplicateSource(*tap));
        }
        let option = sources
            .iter()
            .find(|o| o.source == *tap)
            .ok_or(PaymentError::UnknownSource(*tap))?;
        spent.push(option.produces);
    }
    let got = u32::try_from(spent.len()).unwrap_or(u32::MAX);
    if got != cost.mana_value() {
        return Err(PaymentError::WrongTotal {
            needed: cost.mana_value(),
            got,
        });
    }
    for mana in ManaType::ALL {
        let have = spent.iter().filter(|m| **m == mana).count();
        if have < cost.required(mana) as usize {
            return Err(PaymentError::MissingMana(mana));
        }
    }
    Ok(spent)
}

#[cfg(test)]
mod tests {
    //! Goal: payment validation and auto-payment agree with an independent brute-force affordability check.
    use super::*;
    use mtg_core::mana::ManaSymbol;
    use proptest::prelude::*;
    use rstest::rstest;

    fn pool_of(mana: &[ManaType]) -> ManaPool {
        let mut pool = ManaPool::default();
        for m in mana {
            pool.add(*m);
        }
        pool
    }

    fn lands(types: &str) -> Vec<ManaOption> {
        types
            .chars()
            .enumerate()
            .map(|(i, c)| ManaOption {
                source: ManaSource {
                    object: ObjectId(i as u64),
                    ability: 0,
                },
                produces: ManaType::ALL
                    .into_iter()
                    .find(|t| t.symbol() == c)
                    .expect("valid mana letter"),
            })
            .collect()
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
    fn affordability_table(#[case] cost: &str, #[case] lands_str: &str, #[case] affordable: bool) {
        let cost = ManaCost::parse(cost).unwrap();
        let sources = lands(lands_str);
        let pool = ManaPool::default();
        assert_eq!(can_afford(&cost, &pool, &sources), affordable);
        let auto = auto_payment(&cost, &pool, &sources);
        assert_eq!(auto.is_some(), affordable);
        if let Some(payment) = auto {
            assert!(check_payment(&cost, &pool, &sources, &payment).is_ok());
        }
    }

    #[test]
    fn auto_payment_uses_floating_mana_before_tapping() {
        let cost = ManaCost::parse("{1}{G}").unwrap();
        let pool = pool_of(&[ManaType::Green, ManaType::Red]);
        let sources = lands("GG");
        let payment = auto_payment(&cost, &pool, &sources).unwrap();
        assert_eq!(payment.taps.len(), 0);
        assert_eq!(payment.pool.len(), 2);
    }

    #[test]
    fn auto_payment_keeps_the_scarcer_color_untapped_for_generic() {
        let cost = ManaCost::parse("{1}").unwrap();
        let sources = lands("GGGW");
        let payment = auto_payment(&cost, &ManaPool::default(), &sources).unwrap();
        assert_eq!(payment.taps.len(), 1);
        assert_eq!(
            payment.taps[0].object,
            ObjectId(0),
            "taps a plentiful Forest"
        );
    }

    #[rstest]
    #[case::overpay("{G}", "GG", &[0, 1], "WrongTotal")]
    #[case::underpay("{1}{G}", "GG", &[0], "WrongTotal")]
    #[case::wrong_color("{G}", "WW", &[0], "MissingMana")]
    #[case::unknown_source("{G}", "G", &[7], "UnknownSource")]
    #[case::duplicate("{G}{G}", "GG", &[0, 0], "DuplicateSource")]
    fn bad_payments_are_rejected(
        #[case] cost: &str,
        #[case] lands_str: &str,
        #[case] taps: &[u64],
        #[case] error: &str,
    ) {
        let cost = ManaCost::parse(cost).unwrap();
        let payment = Payment {
            pool: vec![],
            taps: taps
                .iter()
                .map(|o| ManaSource {
                    object: ObjectId(*o),
                    ability: 0,
                })
                .collect(),
        };
        let err = check_payment(&cost, &ManaPool::default(), &lands(lands_str), &payment)
            .expect_err("payment must be rejected");
        assert!(format!("{err:?}").starts_with(error), "{err:?}");
    }

    #[test]
    fn spending_more_floating_mana_than_exists_is_rejected() {
        let cost = ManaCost::parse("{G}").unwrap();
        let payment = Payment {
            pool: vec![ManaType::Green],
            taps: vec![],
        };
        let err = check_payment(&cost, &ManaPool::default(), &[], &payment).unwrap_err();
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

    fn mana_strategy() -> impl Strategy<Value = ManaType> {
        (0usize..6).prop_map(|i| ManaType::ALL[i])
    }

    fn symbol_of(mana: ManaType) -> ManaSymbol {
        mana.color()
            .map_or(ManaSymbol::Colorless, ManaSymbol::Colored)
    }

    /// Tries every subset of floating mana and sources for an exact payment.
    fn brute_force_affordable(
        cost: &ManaCost,
        floating: &[ManaType],
        sources: &[ManaOption],
    ) -> bool {
        let pool = pool_of(floating);
        (0u32..1 << floating.len()).any(|pool_mask| {
            (0u32..1 << sources.len()).any(|source_mask| {
                let payment = Payment {
                    pool: floating
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| (pool_mask >> i) & 1 == 1)
                        .map(|(_, m)| *m)
                        .collect(),
                    taps: sources
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| (source_mask >> i) & 1 == 1)
                        .map(|(_, o)| o.source)
                        .collect(),
                };
                check_payment(cost, &pool, sources, &payment).is_ok()
            })
        })
    }

    proptest! {
        #[test]
        fn auto_payment_exists_iff_affordable_and_is_always_valid(
            generic in 0u32..4,
            costed in prop::collection::vec(mana_strategy(), 0..4),
            floating in prop::collection::vec(mana_strategy(), 0..4),
            source_types in prop::collection::vec(mana_strategy(), 0..7),
        ) {
            let cost = ManaCost::from_symbols(
                std::iter::once(ManaSymbol::Generic(generic)).chain(costed.into_iter().map(symbol_of)),
            );
            let pool = pool_of(&floating);
            let sources: Vec<ManaOption> = source_types
                .iter()
                .zip(0u64..)
                .map(|(t, i)| ManaOption {
                    source: ManaSource { object: ObjectId(i), ability: 0 },
                    produces: *t,
                })
                .collect();
            let affordable = can_afford(&cost, &pool, &sources);
            prop_assert_eq!(affordable, brute_force_affordable(&cost, &floating, &sources));
            let auto = auto_payment(&cost, &pool, &sources);
            prop_assert_eq!(auto.is_some(), affordable);
            if let Some(payment) = auto {
                prop_assert!(check_payment(&cost, &pool, &sources, &payment).is_ok());
            }
        }
    }
}
