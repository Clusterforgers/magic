//! Paying with sources that add several mana or are sacrificed (a Sol Ring, a Gaea's Cradle, a Treasure).
//!
//! A source of one mana for a tap is matched to the coloured needs one for one (`mana.rs`). One that adds several
//! mana covers several needs of its type at once, and a surplus floats, so the choice of sources is a small search:
//! each permanent is left alone or used for one of its options, and a plan is better with fewer sacrifices, then
//! fewer taps, then taps of the types the player has the most sources of.

use std::collections::BTreeMap;

use mtg_core::mana::{ManaCost, ManaType};

use crate::error::PaymentError;
use crate::mana::{ManaOption, ManaSource};

/// Coloured and colourless mana still needed, by type (`ManaType::index`).
pub(crate) type Needs = [u32; 6];

/// What is still needed of the sources: each type's requirement and the mana of any kind in all.
type State = (Needs, u32);

/// How much a plan is wanted, lower first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Weight {
    /// Sources sacrificed.
    sacrifices: u32,
    /// Sources that do more than add mana.
    extras: u32,
    /// Permanents tapped or sacrificed.
    taps: u32,
    /// Sources used whose mana has no restriction, which is of more use than mana with one.
    unrestricted: u32,
    /// How far the types used are from the commonest ones.
    scarcity: u32,
}

impl Weight {
    /// The weight with `option` used too, among `all` sources of which `plenty` make the type it produces.
    pub(crate) fn with(self, option: &ManaOption, all: u32, plenty: u32) -> Weight {
        Weight {
            sacrifices: self.sacrifices + u32::from(option.sacrifice),
            extras: self.extras + u32::from(option.extra),
            taps: self.taps + 1,
            unrestricted: self.unrestricted + u32::from(option.only.is_empty()),
            scarcity: self.scarcity + all.saturating_sub(plenty),
        }
    }
}

/// The sources to use so that the options they are used for make `needs` of each type and `generic` more mana of any
/// type, at most one option of each permanent; `None` when there are not enough.
pub(crate) fn plan(
    needs: Needs,
    generic: u32,
    groups: &[Vec<ManaOption>],
    sources: &[ManaOption],
) -> Option<Vec<ManaSource>> {
    let all = u32::try_from(sources.len()).unwrap_or(u32::MAX);
    let plenty = |mana: ManaType| {
        u32::try_from(sources.iter().filter(|o| o.produces == mana).count()).unwrap_or(u32::MAX)
    };
    let start: State = (needs, needs.iter().sum::<u32>() + generic);
    let mut best: BTreeMap<State, (Weight, Vec<ManaSource>)> = BTreeMap::new();
    best.insert(start, (Weight::default(), Vec::new()));
    for group in groups {
        let mut next = best.clone();
        for (state, (weight, picks)) in &best {
            for option in group {
                let mut left = state.0;
                let made = option.made();
                for mana in &made {
                    let slot = &mut left[mana.index()];
                    *slot = slot.saturating_sub(1);
                }
                let amount = u32::try_from(made.len()).unwrap_or(u32::MAX);
                let after: State = (left, state.1.saturating_sub(amount));
                let used = weight.with(option, all, plenty(option.produces));
                if next.get(&after).is_none_or(|(held, _)| used < *held) {
                    let mut chosen = picks.clone();
                    chosen.push(option.source);
                    next.insert(after, (used, chosen));
                }
            }
        }
        best = next;
    }
    best.remove(&([0; 6], 0)).map(|(_, picks)| picks)
}

/// The mana a payment spends on the plain `cost`, given what each of its parts makes (one entry for each mana of the
/// floating pool, one for each tap).
///
/// A payment must cover the cost and have nothing to spare: no part can be left out and still cover it. What
/// a part made beyond the cost floats. A payment made of single mana spends all of it, so it must be the cost's
/// total exactly.
pub(crate) fn spent_of(
    cost: &ManaCost,
    parts: &[Vec<ManaType>],
) -> Result<Vec<ManaType>, PaymentError> {
    let made: Vec<ManaType> = parts.concat();
    let value = cost.mana_value();
    let got = u32::try_from(made.len()).unwrap_or(u32::MAX);
    let singles = parts.iter().all(|part| part.len() == 1);
    if got < value || (singles && got != value) {
        return Err(PaymentError::WrongTotal { needed: value, got });
    }
    if singles {
        covers(cost, &made, value)?;
        return Ok(made);
    }
    covers(cost, &made, value)?;
    for part in parts {
        let rest: Vec<ManaType> = remove(&made, part);
        if covers(cost, &rest, value).is_ok() {
            return Err(PaymentError::WrongTotal { needed: value, got });
        }
    }
    Ok(select(cost, &made))
}

/// `made` without one of each mana in `part`.
fn remove(made: &[ManaType], part: &[ManaType]) -> Vec<ManaType> {
    let mut rest = made.to_vec();
    for mana in part {
        if let Some(at) = rest.iter().position(|m| m == mana) {
            rest.remove(at);
        }
    }
    rest
}

/// Whether `made` covers the plain `cost` whose total is `value`: enough mana, and each type's requirement.
fn covers(cost: &ManaCost, made: &[ManaType], value: u32) -> Result<(), PaymentError> {
    let got = u32::try_from(made.len()).unwrap_or(u32::MAX);
    if got < value {
        return Err(PaymentError::WrongTotal { needed: value, got });
    }
    for mana in ManaType::ALL {
        let have = made.iter().filter(|m| **m == mana).count();
        if have < cost.required(mana) as usize {
            return Err(PaymentError::MissingMana(mana));
        }
    }
    Ok(())
}

/// The mana of `made` that pays `cost`, in the order made: each type's requirement first, then any for the generic part.
pub(crate) fn select(cost: &ManaCost, made: &[ManaType]) -> Vec<ManaType> {
    let mut wanted: Needs = [0; 6];
    for mana in ManaType::ALL {
        wanted[mana.index()] = cost.required(mana);
    }
    let (mut spent, mut spare) = (Vec::new(), Vec::new());
    for &mana in made {
        let slot = &mut wanted[mana.index()];
        if *slot > 0 {
            *slot -= 1;
            spent.push(mana);
        } else {
            spare.push(mana);
        }
    }
    let generic = (cost.mana_value() as usize).saturating_sub(spent.len());
    spent.extend(spare.into_iter().take(generic));
    spent
}

/// What is left to pay of the plain `cost` after the parts of a payment made so far: the requirements not yet met and
/// the mana of any kind still missing. A payment with something to spare is refused as wasted.
pub(crate) fn rest_after(
    cost: &ManaCost,
    parts: &[Vec<ManaType>],
) -> Result<(Needs, u32), PaymentError> {
    let made: Vec<ManaType> = parts.concat();
    let value = cost.mana_value();
    let got = u32::try_from(made.len()).unwrap_or(u32::MAX);
    for part in parts {
        if covers(cost, &remove(&made, part), value).is_ok() {
            return Err(PaymentError::Wasted);
        }
    }
    let mut needs: Needs = [0; 6];
    let mut covered = 0;
    for mana in ManaType::ALL {
        let have = u32::try_from(made.iter().filter(|m| **m == mana).count()).unwrap_or(u32::MAX);
        let required = cost.required(mana);
        needs[mana.index()] = required.saturating_sub(have);
        covered += have.min(required);
    }
    // Single mana that overflow the generic part can never be completed; a part of several mana may float its surplus.
    if parts.iter().all(|part| part.len() == 1) && got - covered > cost.generic() {
        return Err(PaymentError::Wasted);
    }
    let generic = cost.generic().saturating_sub(got - covered);
    Ok((needs, generic))
}

/// Whether the mana of `parts` covers the plain `cost`: enough in all and of each type.
pub(crate) fn covers_cost(cost: &ManaCost, parts: &[Vec<ManaType>]) -> bool {
    covers(cost, &parts.concat(), cost.mana_value()).is_ok()
}

/// The best completion so far: its weight (sacrifices, side effects, taps), the floating mana it spends and the sources it taps.
type Found = ((u32, u32, usize, usize), Vec<ManaType>, Vec<ManaSource>);

/// What a completion may add: one floating mana, or one permanent used for one of its options.
enum Candidate<'a> {
    Floating(ManaType),
    Permanent(&'a [ManaOption]),
}

/// The most nodes [`complete_with`] looks at before it gives up.
const SEARCH_BUDGET: usize = 200_000;

/// The floating mana to spend and the sources to use, on top of the `kept` parts of a payment, so that the whole pays
/// the plain `cost` with nothing to spare; the one with the fewest sacrifices, then the fewest sources. `None` when
/// there is none within the search budget.
pub(crate) fn complete_with(
    cost: &ManaCost,
    kept: &[Vec<ManaType>],
    floating: &[ManaType],
    groups: &[Vec<ManaOption>],
) -> Option<(Vec<ManaType>, Vec<ManaSource>)> {
    struct Search<'a> {
        cost: &'a ManaCost,
        candidates: Vec<Candidate<'a>>,
        parts: Vec<Vec<ManaType>>,
        pool: Vec<ManaType>,
        taps: Vec<ManaOption>,
        best: Option<Found>,
        budget: usize,
    }
    impl Search<'_> {
        fn visit(&mut self, from: usize) {
            if self.budget == 0 {
                return;
            }
            self.budget -= 1;
            if covers_cost(self.cost, &self.parts) {
                if spent_of(self.cost, &self.parts).is_ok() {
                    let sacrifices =
                        u32::try_from(self.taps.iter().filter(|o| o.sacrifice).count())
                            .unwrap_or(u32::MAX);
                    let extras = u32::try_from(self.taps.iter().filter(|o| o.extra).count())
                        .unwrap_or(u32::MAX);
                    let unrestricted = self.taps.iter().filter(|o| o.only.is_empty()).count();
                    let weight = (sacrifices, extras, self.taps.len(), unrestricted);
                    if self.best.as_ref().is_none_or(|(held, ..)| weight < *held) {
                        let sources = self.taps.iter().map(|o| o.source).collect();
                        self.best = Some((weight, self.pool.clone(), sources));
                    }
                }
                // Anything added to a payment that covers the cost has a part to spare.
                return;
            }
            for at in from..self.candidates.len() {
                match &self.candidates[at] {
                    Candidate::Floating(mana) => {
                        let mana = *mana;
                        self.parts.push(vec![mana]);
                        self.pool.push(mana);
                        self.visit(at + 1);
                        self.pool.pop();
                        self.parts.pop();
                    }
                    Candidate::Permanent(options) => {
                        let options = options.to_vec();
                        for option in options {
                            self.parts.push(option.made());
                            self.taps.push(option);
                            self.visit(at + 1);
                            self.taps.pop();
                            self.parts.pop();
                        }
                    }
                }
            }
        }
    }
    let mut candidates: Vec<Candidate<'_>> = Vec::new();
    candidates.extend(floating.iter().map(|m| Candidate::Floating(*m)));
    candidates.extend(groups.iter().map(|g| Candidate::Permanent(g)));
    let mut search = Search {
        cost,
        candidates,
        parts: kept.to_vec(),
        pool: Vec::new(),
        taps: Vec::new(),
        best: None,
        budget: SEARCH_BUDGET,
    };
    search.visit(0);
    search.best.map(|(_, pool, taps)| (pool, taps))
}

#[cfg(test)]
mod tests {
    //! Goal: sources that add several mana or are sacrificed are chosen to cover a cost with the fewest sacrifices and taps, a surplus is allowed only from the last part needed, and a payment with a part to spare is refused.
    use mtg_core::ids::ObjectId;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;

    fn option(object: u64, produces: ManaType, count: u32, sacrifice: bool) -> ManaOption {
        ManaOption {
            source: ManaSource {
                object: ObjectId(object),
                ability: 0,
            },
            produces,
            count,
            sacrifice,
            extra: false,
            only: Vec::new(),
            also: Vec::new(),
            life: 0,
            energy: 0,
            energy_held: 0,
            cost: ManaCost::ZERO,
        }
    }

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

    fn needs_of(cost: &ManaCost) -> (Needs, u32) {
        let mut needs: Needs = [0; 6];
        for mana in ManaType::ALL {
            needs[mana.index()] = cost.required(mana);
        }
        (needs, cost.generic())
    }

    fn picked(cost_text: &str, sources: &[ManaOption]) -> Option<Vec<u64>> {
        let (needs, generic) = needs_of(&cost(cost_text));
        let mut groups: Vec<Vec<ManaOption>> = Vec::new();
        for option in sources {
            match groups
                .iter_mut()
                .find(|g| g[0].source.object == option.source.object)
            {
                Some(group) => group.push(option.clone()),
                None => groups.push(vec![option.clone()]),
            }
        }
        plan(needs, generic, &groups, sources).map(|picks| {
            let mut objects: Vec<u64> = picks.iter().map(|p| p.object.0).collect();
            objects.sort_unstable();
            objects
        })
    }

    #[rstest]
    #[case::one_ring_pays_two("{2}", &[option(1, ManaType::Colorless, 2, false), option(2, ManaType::Green, 1, false), option(3, ManaType::Green, 1, false)], Some(vec![1]))]
    #[case::the_ring_and_a_land_pay_three("{3}", &[option(1, ManaType::Colorless, 2, false), option(2, ManaType::Green, 1, false), option(3, ManaType::Green, 1, false)], Some(vec![1, 2]))]
    #[case::colour_from_the_land("{G}{1}", &[option(1, ManaType::Colorless, 2, false), option(2, ManaType::Green, 1, false)], Some(vec![1, 2]))]
    #[case::not_enough("{4}", &[option(1, ManaType::Colorless, 2, false), option(2, ManaType::Green, 1, false)], None)]
    #[case::cradle_counts_for_each("{G}{G}{G}", &[option(1, ManaType::Green, 3, false)], Some(vec![1]))]
    #[case::a_colour_a_ring_cannot_make("{U}{1}", &[option(1, ManaType::Colorless, 2, false)], None)]
    #[case::a_land_before_a_treasure("{1}", &[option(1, ManaType::Green, 1, true), option(2, ManaType::Green, 1, false)], Some(vec![2]))]
    #[case::a_treasure_when_the_land_is_not_enough("{2}", &[option(1, ManaType::Green, 1, true), option(2, ManaType::Green, 1, false)], Some(vec![1, 2]))]
    #[case::fewest_sacrifices("{2}", &[option(1, ManaType::Green, 1, true), option(2, ManaType::Green, 1, true), option(3, ManaType::Colorless, 2, false)], Some(vec![3]))]
    #[case::a_treasure_of_the_colour_needed("{U}", &[option(1, ManaType::Blue, 1, true), option(1, ManaType::Red, 1, true), option(2, ManaType::Green, 1, false)], Some(vec![1]))]
    fn the_cheapest_plan_covers_the_needs(
        #[case] cost_text: &str,
        #[case] sources: &[ManaOption],
        #[case] expected: Option<Vec<u64>>,
    ) {
        assert_eq!(picked(cost_text, sources), expected);
    }

    #[rstest]
    #[case::exact("{G}{1}", vec![mana("G"), mana("C")], Some("GC"))]
    #[case::a_surplus_floats("{1}", vec![mana("CC")], Some("C"))]
    #[case::one_source_of_two_colours("{U}{1}", vec![mana("UU")], Some("UU"))]
    #[case::single_mana_stay_in_order("{U}{1}", vec![mana("G"), mana("U")], Some("GU"))]
    #[case::a_ring_for_three("{3}", vec![mana("CC"), mana("G")], Some("CCG"))]
    #[case::a_part_to_spare("{2}", vec![mana("CC"), mana("G")], None)]
    #[case::two_lands_for_one("{1}", vec![mana("G"), mana("G")], None)]
    #[case::too_little("{3}", vec![mana("CC")], None)]
    #[case::missing_colour("{U}{1}", vec![mana("CC")], None)]
    fn a_payment_covers_the_cost_with_nothing_to_spare(
        #[case] cost_text: &str,
        #[case] parts: Vec<Vec<ManaType>>,
        #[case] spent: Option<&str>,
    ) {
        let got = spent_of(&cost(cost_text), &parts).ok();
        assert_eq!(got, spent.map(mana));
    }

    #[rstest]
    #[case::a_ring_leaves_one("{3}", vec![mana("CC")], Ok(([0; 6], 1)))]
    #[case::a_colour_is_left("{G}{G}{1}", vec![mana("G")], Ok(([0, 0, 0, 0, 1, 0], 1)))]
    #[case::covered("{2}", vec![mana("CC")], Ok(([0; 6], 0)))]
    #[case::wasted("{1}", vec![mana("G"), mana("G")], Err(PaymentError::Wasted))]
    #[case::a_part_to_spare("{2}", vec![mana("CC"), mana("G")], Err(PaymentError::Wasted))]
    fn what_is_left_after_a_partial_payment(
        #[case] cost_text: &str,
        #[case] parts: Vec<Vec<ManaType>>,
        #[case] expected: Result<(Needs, u32), PaymentError>,
    ) {
        assert_eq!(rest_after(&cost(cost_text), &parts), expected);
    }
}
