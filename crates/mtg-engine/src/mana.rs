//! Mana pool and payment: validating a chosen payment and computing an automatic one.

use std::collections::BTreeMap;

use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType};
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

/// The sources grouped by permanent: a permanent is tapped once, for one of its abilities.
///
/// Permanents with fewer abilities come first, so a flexible permanent is the last one tapped for a fixed need.
fn by_permanent(sources: &[ManaOption]) -> Vec<Vec<ManaOption>> {
    let mut groups: BTreeMap<ObjectId, Vec<ManaOption>> = BTreeMap::new();
    for option in sources {
        groups
            .entry(option.source.object)
            .or_default()
            .push(*option);
    }
    let mut groups: Vec<Vec<ManaOption>> = groups.into_values().collect();
    for group in &mut groups {
        group.sort_by_key(|o| o.source);
    }
    groups.sort_by_key(|g| (g.len(), g.first().map(|o| o.source)));
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

/// Whether `cost` can be paid exactly from the pool plus the given sources, tapping each permanent once.
pub fn can_afford(cost: &ManaCost, pool: &ManaPool, sources: &[ManaOption]) -> bool {
    let groups = by_permanent(sources);
    let mut needs = Vec::new();
    for mana in ManaType::ALL {
        let missing = cost.required(mana).saturating_sub(pool.get(mana));
        needs.extend(std::iter::repeat_n(mana, missing as usize));
    }
    let supply = u64::from(pool.total()) + groups.len() as u64;
    supply >= u64::from(cost.mana_value()) && match_needs(&needs, &groups).is_some()
}

/// Picks a valid payment if one exists: floating mana first, then sources, keeping scarce colors untapped.
pub fn auto_payment(cost: &ManaCost, pool: &ManaPool, sources: &[ManaOption]) -> Option<Payment> {
    let mut pool_left = *pool;
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
    for mana in ManaType::ALL {
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

/// The mana a payment's parts make, after checking each part on its own.
///
/// The floating mana must exist, and every tap must be an offered ability of a permanent that is not
/// tapped, used once: a second ability of the same permanent would tap it again.
fn spent_by_parts(
    pool: &ManaPool,
    sources: &[ManaOption],
    tapped: &[ObjectId],
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
        if tapped.contains(&tap.object) {
            return Err(PaymentError::AlreadyTapped(tap.object));
        }
        let option = sources
            .iter()
            .find(|o| o.source == *tap)
            .ok_or(PaymentError::UnknownSource(*tap))?;
        if payment.taps[..i].iter().any(|t| t.object == tap.object) {
            return Err(PaymentError::ObjectTappedTwice(tap.object));
        }
        spent.push(option.produces);
    }
    Ok(spent)
}

/// Validates an explicit payment and returns the mana it spends.
///
/// The payment must cover the cost exactly: no overpaying, no missing colored mana. `tapped` lists
/// permanents that are already tapped, which no payment may use.
pub fn check_payment(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    tapped: &[ObjectId],
    payment: &Payment,
) -> Result<Vec<ManaType>, PaymentError> {
    let spent = spent_by_parts(pool, sources, tapped, payment)?;
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

/// Finishes a partial payment: keeps every part of `partial` and adds the rest automatically.
///
/// The parts are checked like a full payment's, and they must fit inside the cost (mana beyond the
/// coloured requirements has to go to the generic part). The result passes [`check_payment`].
pub fn complete_payment(
    cost: &ManaCost,
    pool: &ManaPool,
    sources: &[ManaOption],
    tapped: &[ObjectId],
    partial: &Payment,
) -> Result<Payment, PaymentError> {
    let spent = spent_by_parts(pool, sources, tapped, partial)?;
    let mut generic = cost.generic();
    let mut symbols = Vec::new();
    for mana in ManaType::ALL {
        let have = u32::try_from(spent.iter().filter(|m| **m == mana).count()).unwrap_or(u32::MAX);
        let need = cost.required(mana);
        generic = generic
            .checked_sub(have.saturating_sub(need))
            .ok_or(PaymentError::Wasted)?;
        let symbol = mana
            .color()
            .map_or(ManaSymbol::Colorless, ManaSymbol::Colored);
        symbols.extend(std::iter::repeat_n(
            symbol,
            need.saturating_sub(have) as usize,
        ));
    }
    let rest_cost =
        ManaCost::from_symbols(std::iter::once(ManaSymbol::Generic(generic)).chain(symbols));
    let mut rest_pool = *pool;
    for &mana in &partial.pool {
        rest_pool.spend(mana);
    }
    let rest_sources: Vec<ManaOption> = sources
        .iter()
        .filter(|o| !partial.taps.iter().any(|t| t.object == o.source.object))
        .copied()
        .collect();
    let rest =
        auto_payment(&rest_cost, &rest_pool, &rest_sources).ok_or(PaymentError::CannotComplete)?;
    let whole = Payment {
        pool: [partial.pool.as_slice(), rest.pool.as_slice()].concat(),
        taps: [partial.taps.as_slice(), rest.taps.as_slice()].concat(),
    };
    check_payment(cost, pool, sources, tapped, &whole)?;
    Ok(whole)
}

#[cfg(test)]
mod tests {
    //! Goal: payment validation, auto-payment and completion of partial payments agree with an independent brute-force check, tapping each permanent once.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

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
                letters
                    .chars()
                    .enumerate()
                    .map(move |(ability, c)| ManaOption {
                        source: ManaSource {
                            object: ObjectId(object),
                            ability,
                        },
                        produces: mana_of(c),
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
            pool: vec![],
            taps: of.iter().map(|(o, a)| tap(*o, *a)).collect(),
        }
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
            assert!(check_payment(&cost, &pool, &sources, &[], &payment).is_ok());
        }
    }

    #[rstest]
    #[case::one_permanent_cannot_pay_two("{W}{G}", &["GW"], false)]
    #[case::second_permanent_covers_the_other_colour("{W}{G}", &["GW", "G"], true)]
    #[case::flexible_permanent_serves_either("{W}", &["GW"], true)]
    #[case::two_flexible_permanents_cover_a_colour_each("{W}{G}", &["GW", "GW"], true)]
    #[case::two_flexible_permanents_not_three_mana("{W}{G}{U}", &["GW", "GW"], false)]
    #[case::permanents_are_counted_not_abilities("{3}", &["GW", "G"], false)]
    #[case::each_permanent_is_one_mana("{2}", &["GW", "G"], true)]
    #[case::more_mana_than_permanents("{R}{G}{G}", &["RG", "RW"], false)]
    #[case::a_later_need_moves_an_earlier_choice("{R}{G}", &["RG", "RW"], true)]
    fn permanents_with_several_abilities_tap_once(
        #[case] cost: &str,
        #[case] spec: &[&str],
        #[case] affordable: bool,
    ) {
        let cost = ManaCost::parse(cost).unwrap();
        let sources = permanents(spec);
        let pool = ManaPool::default();
        assert_eq!(can_afford(&cost, &pool, &sources), affordable);
        let auto = auto_payment(&cost, &pool, &sources);
        assert_eq!(auto.is_some(), affordable);
        if let Some(payment) = auto {
            assert_eq!(
                check_payment(&cost, &pool, &sources, &[], &payment).map(|_| ()),
                Ok(())
            );
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
    #[case::overpay("{G}", "GG", &[], &[(0, 0), (1, 0)], "WrongTotal")]
    #[case::underpay("{1}{G}", "GG", &[], &[(0, 0)], "WrongTotal")]
    #[case::wrong_color("{G}", "WW", &[], &[(0, 0)], "MissingMana")]
    #[case::unknown_source("{G}", "G", &[], &[(7, 0)], "UnknownSource")]
    #[case::unknown_ability("{G}", "G", &[], &[(0, 3)], "UnknownSource")]
    #[case::duplicate("{G}{G}", "GG", &[], &[(0, 0), (0, 0)], "DuplicateSource")]
    #[case::already_tapped("{G}", "G", &[0], &[(0, 0)], "AlreadyTapped")]
    #[case::one_of_two_already_tapped("{G}{G}", "GGG", &[1], &[(0, 0), (1, 0)], "AlreadyTapped")]
    #[case::already_tapped_and_not_offered("{G}", "W", &[5], &[(5, 0)], "AlreadyTapped")]
    fn bad_payments_are_rejected(
        #[case] cost: &str,
        #[case] lands_str: &str,
        #[case] tapped: &[u64],
        #[case] taps_of: &[(u64, usize)],
        #[case] error: &str,
    ) {
        let cost = ManaCost::parse(cost).unwrap();
        let tapped: Vec<ObjectId> = tapped.iter().map(|o| ObjectId(*o)).collect();
        let err = check_payment(
            &cost,
            &ManaPool::default(),
            &lands(lands_str),
            &tapped,
            &taps(taps_of),
        )
        .expect_err("payment must be rejected");
        assert!(format!("{err:?}").starts_with(error), "{err:?}");
    }

    #[rstest]
    #[case::two_abilities_two_colours("{W}{G}", &["GW"], &[(0, 0), (0, 1)])]
    #[case::two_abilities_one_colour("{G}{G}", &["GG"], &[(0, 0), (0, 1)])]
    #[case::second_ability_after_another_tap("{G}{W}{U}", &["G", "GW", "U"], &[(0, 0), (1, 0), (1, 1)])]
    #[case::interleaved_taps("{1}{1}", &["GW", "GW"], &[(0, 0), (1, 0), (0, 1)])]
    fn two_abilities_of_one_permanent_are_rejected(
        #[case] cost: &str,
        #[case] spec: &[&str],
        #[case] taps_of: &[(u64, usize)],
    ) {
        let cost = ManaCost::parse(cost).unwrap();
        let sources = permanents(spec);
        let err = check_payment(&cost, &ManaPool::default(), &sources, &[], &taps(taps_of))
            .expect_err("a permanent cannot be tapped twice");
        assert!(matches!(err, PaymentError::ObjectTappedTwice(_)), "{err:?}");
    }

    #[test]
    fn one_ability_each_of_two_permanents_is_accepted() {
        let cost = ManaCost::parse("{W}{G}").unwrap();
        let sources = permanents(&["GW", "GW"]);
        let ok = check_payment(
            &cost,
            &ManaPool::default(),
            &sources,
            &[],
            &taps(&[(0, 1), (1, 0)]),
        );
        assert_eq!(ok, Ok(vec![ManaType::White, ManaType::Green]));
    }

    #[test]
    fn spending_more_floating_mana_than_exists_is_rejected() {
        let cost = ManaCost::parse("{G}").unwrap();
        let payment = Payment {
            pool: vec![ManaType::Green],
            taps: vec![],
        };
        let err = check_payment(&cost, &ManaPool::default(), &[], &[], &payment).unwrap_err();
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
        cost: &str,
        floating: &str,
        spec: &[&str],
        tapped: &[u64],
        partial: &Payment,
    ) -> Result<Payment, PaymentError> {
        let cost = ManaCost::parse(cost).unwrap();
        let pool = pool_of(&floating.chars().map(mana_of).collect::<Vec<_>>());
        let sources = permanents(spec);
        let tapped: Vec<ObjectId> = tapped.iter().map(|o| ObjectId(*o)).collect();
        let whole = complete_payment(&cost, &pool, &sources, &tapped, partial)?;
        assert_eq!(
            check_payment(&cost, &pool, &sources, &tapped, &whole).map(|_| ()),
            Ok(()),
            "a completion is a valid payment"
        );
        assert!(
            partial.taps.iter().all(|t| whole.taps.contains(t)),
            "the chosen taps are kept"
        );
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
    fn completion_pays_the_rest(
        #[case] cost: &str,
        #[case] floating: &str,
        #[case] spec: &[&str],
        #[case] tapped: &[u64],
        #[case] partial: &[(u64, usize)],
        #[case] pieces: usize,
    ) {
        let whole = completed(cost, floating, spec, tapped, &taps(partial)).unwrap();
        assert_eq!(whole.pool.len() + whole.taps.len(), pieces);
    }

    #[test]
    fn completion_keeps_chosen_floating_mana() {
        let partial = Payment {
            pool: vec![ManaType::Red],
            taps: vec![],
        };
        let whole = completed("{1}{G}", "RR", &["G", "W"], &[], &partial).unwrap();
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
    fn completion_refuses_what_cannot_be_finished(
        #[case] cost: &str,
        #[case] spec: &[&str],
        #[case] partial: &[(u64, usize)],
        #[case] error: PaymentError,
    ) {
        assert_eq!(completed(cost, "", spec, &[], &taps(partial)), Err(error));
    }

    #[test]
    fn completion_refuses_a_tapped_permanent_and_missing_floating_mana() {
        let tapped = completed("{G}", "", &["G"], &[0], &taps(&[(0, 0)]));
        assert_eq!(tapped, Err(PaymentError::AlreadyTapped(ObjectId(0))));
        let partial = Payment {
            pool: vec![ManaType::Red],
            taps: vec![],
        };
        let short = completed("{R}", "", &["R"], &[], &partial);
        assert_eq!(short, Err(PaymentError::PoolShortfall(ManaType::Red)));
    }

    fn mana_strategy() -> impl Strategy<Value = ManaType> {
        (0usize..6).prop_map(|i| ManaType::ALL[i])
    }

    fn symbol_of(mana: ManaType) -> ManaSymbol {
        mana.color()
            .map_or(ManaSymbol::Colorless, ManaSymbol::Colored)
    }

    /// Tries every subset of floating mana and sources for an exact payment that contains `partial`.
    fn brute_force(
        cost: &ManaCost,
        floating: &[ManaType],
        sources: &[ManaOption],
        partial: &Payment,
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
                let holds_partial = partial.taps.iter().all(|t| payment.taps.contains(t))
                    && ManaType::ALL.into_iter().all(|m| {
                        let wanted = partial.pool.iter().filter(|p| **p == m).count();
                        payment.pool.iter().filter(|p| **p == m).count() >= wanted
                    });
                holds_partial && check_payment(cost, &pool, sources, &[], &payment).is_ok()
            })
        })
    }

    /// A random scenario: a cost, floating mana, and permanents with one or two mana abilities each.
    fn scenario() -> impl Strategy<Value = (ManaCost, Vec<ManaType>, Vec<ManaOption>)> {
        (
            0u32..4,
            prop::collection::vec(mana_strategy(), 0..4),
            prop::collection::vec(mana_strategy(), 0..4),
            prop::collection::vec(prop::collection::vec(mana_strategy(), 1..3), 0..5),
        )
            .prop_map(|(generic, costed, floating, permanents)| {
                let cost = ManaCost::from_symbols(
                    std::iter::once(ManaSymbol::Generic(generic))
                        .chain(costed.into_iter().map(symbol_of)),
                );
                let mut sources = Vec::new();
                for (object, abilities) in (0u64..).zip(permanents) {
                    for (ability, produces) in abilities.into_iter().enumerate() {
                        sources.push(ManaOption {
                            source: tap(object, ability),
                            produces,
                        });
                    }
                }
                sources.truncate(8);
                (cost, floating, sources)
            })
    }

    proptest! {
        #[test]
        fn auto_payment_exists_iff_affordable_and_is_always_valid(
            (cost, floating, sources) in scenario(),
        ) {
            let pool = pool_of(&floating);
            let affordable = can_afford(&cost, &pool, &sources);
            prop_assert_eq!(affordable, brute_force(&cost, &floating, &sources, &Payment::default()));
            let auto = auto_payment(&cost, &pool, &sources);
            prop_assert_eq!(auto.is_some(), affordable);
            if let Some(payment) = auto {
                prop_assert_eq!(check_payment(&cost, &pool, &sources, &[], &payment).map(|_| ()), Ok(()));
            }
        }

        #[test]
        fn completion_exists_iff_some_valid_payment_contains_the_partial(
            (cost, floating, sources) in scenario(),
            pool_mask in any::<u8>(),
            source_mask in any::<u8>(),
        ) {
            let pool = pool_of(&floating);
            let partial = Payment {
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
            let done = complete_payment(&cost, &pool, &sources, &[], &partial);
            prop_assert_eq!(done.is_ok(), brute_force(&cost, &floating, &sources, &partial));
            if let Ok(whole) = done {
                prop_assert_eq!(check_payment(&cost, &pool, &sources, &[], &whole).map(|_| ()), Ok(()));
                prop_assert!(partial.taps.iter().all(|t| whole.taps.contains(t)));
                prop_assert!(whole.pool.len() >= partial.pool.len());
            }
        }
    }
}
