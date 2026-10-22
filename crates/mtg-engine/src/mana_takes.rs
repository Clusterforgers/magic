//! The permanents a mana ability takes as part of its cost (Springleaf Drum taps a creature, Skirk Prospector
//! sacrifices a Goblin) and the choice of which ones a payment takes.
//!
//! A source with such a cost lists, for each part, the permanents it may take (`ManaOption::takes`). A payment is
//! valid when every part gets as many permanents as it takes, no permanent is taken twice and no permanent that another
//! source of the payment uses for its mana is taken. That keeps the order of the activations out of the rules: a
//! permanent that makes mana first and is taken after is activated by hand before the payment. The engine picks the
//! permanents (`assignment`); [`solving`] finds the payment that is wanted most among those whose sources can all take.

use mtg_core::ids::ObjectId;

use crate::mana::{ManaOption, ManaSource, Payment};

/// The most plans [`solving`] tries beyond the first.
const BUDGET: usize = 256;

/// One permanent a part of a cost must take: the owner (an index among the options), the part and what it may take.
struct Slot {
    /// Index of the option the slot belongs to.
    owner: usize,
    /// Index of the part among the option's `takes`.
    part: usize,
    /// The permanents that may fill it.
    from: Vec<ObjectId>,
}

/// The permanents each of `options` takes for each of its parts, or `None` when they cannot all take.
pub(crate) fn assignment(options: &[&ManaOption]) -> Option<Vec<Vec<Vec<ObjectId>>>> {
    let mut slots = Vec::new();
    for (owner, option) in options.iter().enumerate() {
        for (part, take) in option.takes.iter().enumerate() {
            let from: Vec<ObjectId> = take
                .from
                .iter()
                .copied()
                .filter(|id| {
                    !options
                        .iter()
                        .enumerate()
                        .any(|(other, o)| other != owner && o.source.object == *id)
                })
                .collect();
            for _ in 0..take.count {
                slots.push(Slot {
                    owner,
                    part,
                    from: from.clone(),
                });
            }
        }
    }
    // The narrowest slots first fail soonest.
    let mut order: Vec<usize> = (0..slots.len()).collect();
    order.sort_by_key(|&slot| slots[slot].from.len());
    let mut chosen = vec![None; slots.len()];
    if !fill(&order, &slots, &mut chosen) {
        return None;
    }
    let mut taken: Vec<Vec<Vec<ObjectId>>> = options
        .iter()
        .map(|option| vec![Vec::new(); option.takes.len()])
        .collect();
    for (slot, id) in slots.iter().zip(chosen) {
        taken[slot.owner][slot.part].extend(id);
    }
    Some(taken)
}

/// Fills the slots of `order` one after the other with permanents no other slot has.
fn fill(order: &[usize], slots: &[Slot], chosen: &mut Vec<Option<ObjectId>>) -> bool {
    let Some((&slot, rest)) = order.split_first() else {
        return true;
    };
    for &id in &slots[slot].from {
        if chosen.contains(&Some(id)) {
            continue;
        }
        chosen[slot] = Some(id);
        if fill(rest, slots, chosen) {
            return true;
        }
        chosen[slot] = None;
    }
    false
}

/// The options a payment uses: the `held` ones and the sources of `payment`.
fn used<'a>(
    held: &[&'a ManaOption],
    sources: &'a [ManaOption],
    payment: &Payment,
) -> Vec<&'a ManaOption> {
    let mut used = held.to_vec();
    for tap in &payment.taps {
        if used.iter().any(|option| option.source == *tap) {
            continue;
        }
        used.extend(sources.iter().find(|option| option.source == *tap));
    }
    used
}

/// The payment `plan` makes from `sources` when every source of it (and the `held` ones, already chosen) can take what
/// its cost takes; otherwise the most wanted (`better`) of the payments `plan` makes without one of the sources that
/// cannot, searched until none is left or the budget is spent.
///
/// A payment with a source that cannot take has a smallest set of sources that cannot all take; any valid payment
/// leaves one of them out, so every way to leave one out is tried, and the sources that stand in for it (the same
/// permanent with the same parts) with it.
pub(crate) fn solving(
    held: &[&ManaOption],
    sources: &[ManaOption],
    plan: &dyn Fn(&[ManaOption]) -> Option<Payment>,
    better: &dyn Fn(&Payment, &Payment) -> bool,
) -> Option<Payment> {
    if held
        .iter()
        .copied()
        .chain(sources)
        .all(|option| option.takes.is_empty())
    {
        return plan(sources);
    }
    search(held, sources, plan, better, &mut BUDGET.clone())
}

fn search(
    held: &[&ManaOption],
    sources: &[ManaOption],
    plan: &dyn Fn(&[ManaOption]) -> Option<Payment>,
    better: &dyn Fn(&Payment, &Payment) -> bool,
    budget: &mut usize,
) -> Option<Payment> {
    let payment = plan(sources)?;
    let all = used(held, sources, &payment);
    if assignment(&all).is_some() {
        return Some(payment);
    }
    if *budget == 0 {
        return None;
    }
    *budget -= 1;
    // Drop every source the others can still do without: what is left cannot take, whichever one is removed.
    let mut core = all;
    let mut at = core.len();
    while at > 0 {
        at -= 1;
        let removed = core.remove(at);
        if assignment(&core).is_some() {
            core.insert(at, removed);
        }
    }
    let mut best: Option<Payment> = None;
    for member in core {
        if held.iter().any(|option| option.source == member.source) {
            continue;
        }
        let rest: Vec<ManaOption> = sources
            .iter()
            .filter(|option| {
                !(option.source.object == member.source.object && option.takes == member.takes)
            })
            .cloned()
            .collect();
        if let Some(found) = search(held, &rest, plan, better, budget)
            && best.as_ref().is_none_or(|kept| better(&found, kept))
        {
            best = Some(found);
        }
    }
    best
}

/// Takes `objects` out of `sources`: they make no mana and their costs do not take them, as the cost of an ability or a
/// spell already taps, sacrifices or returns them.
pub(crate) fn leave(sources: &mut Vec<ManaOption>, objects: &[ObjectId]) {
    sources.retain(|option| !objects.contains(&option.source.object));
    for option in sources.iter_mut() {
        for take in &mut option.takes {
            take.from.retain(|id| !objects.contains(id));
        }
    }
}

/// `sources` without the permanents in `gone` among those their costs may take (they were tapped for convoke).
pub(crate) fn without_taken(
    sources: &[ManaOption],
    gone: &dyn Fn(ObjectId) -> bool,
) -> Vec<ManaOption> {
    sources
        .iter()
        .map(|option| {
            let mut option = option.clone();
            for take in &mut option.takes {
                take.from.retain(|id| !gone(*id));
            }
            option
        })
        .collect()
}

/// The sources of `payment` among `sources`, which `assignment` reads.
pub(crate) fn sources_of<'a>(
    sources: &'a [ManaOption],
    taps: &[ManaSource],
) -> Vec<&'a ManaOption> {
    taps.iter()
        .filter_map(|tap| sources.iter().find(|option| option.source == *tap))
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: the permanents the sources of a payment take are found when each part gets as many as it takes, none twice and none that another source of the payment uses for its mana, and a payment whose sources cannot take is planned again without them.
    use mtg_core::mana::ManaType;
    use rstest::rstest;

    use super::*;
    use crate::mana::Take;

    /// A source of the permanent `object` whose parts take `count` of `from` each.
    fn taker(object: u64, takes: &[(u32, &[u64])]) -> ManaOption {
        ManaOption {
            takes: takes
                .iter()
                .map(|(count, from)| Take {
                    sacrifice: false,
                    count: *count,
                    from: from.iter().map(|id| ObjectId(*id)).collect(),
                })
                .collect(),
            ..ManaOption::new(
                ManaSource {
                    object: ObjectId(object),
                    ability: 0,
                },
                ManaType::Red,
            )
        }
    }

    type Parts<'a> = &'a [(u32, &'a [u64])];

    #[rstest]
    #[case::nothing_to_take(&[(1, &[][..])], true)]
    #[case::one_part_one_permanent(&[(1, &[(1, &[7][..])][..])], true)]
    #[case::the_part_has_none(&[(1, &[(1, &[][..])][..])], false)]
    #[case::two_parts_one_permanent(&[(1, &[(1, &[7][..]), (1, &[7][..])][..])], false)]
    #[case::two_parts_two_permanents(&[(1, &[(1, &[7, 8][..]), (1, &[7][..])][..])], true)]
    #[case::two_sources_one_permanent(&[(1, &[(1, &[7][..])][..]), (2, &[(1, &[7][..])][..])], false)]
    #[case::two_sources_two_permanents(&[(1, &[(1, &[7, 8][..])][..]), (2, &[(1, &[7][..])][..])], true)]
    #[case::a_count_of_two(&[(1, &[(2, &[7, 8][..])][..])], true)]
    #[case::a_count_of_two_with_one_permanent(&[(1, &[(2, &[7][..])][..])], false)]
    #[case::the_other_source_makes_its_mana(&[(1, &[(1, &[2][..])][..]), (2, &[][..])], false)]
    #[case::the_source_may_take_itself(&[(1, &[(1, &[1][..])][..]), (2, &[][..])], true)]
    fn permanents_are_taken_once_and_never_from_another_source(
        #[case] sources: &[(u64, Parts<'_>)],
        #[case] taken: bool,
    ) {
        let options: Vec<ManaOption> = sources
            .iter()
            .map(|(object, takes)| taker(*object, takes))
            .collect();
        let found = assignment(&options.iter().collect::<Vec<_>>());
        assert_eq!(found.is_some(), taken);
        if let Some(found) = found {
            let mut all: Vec<ObjectId> = found.concat().concat();
            let total = all.len();
            all.sort();
            all.dedup();
            assert_eq!(all.len(), total, "no permanent twice");
        }
    }

    #[test]
    fn a_plan_that_cannot_take_is_made_again_without_the_source_that_cannot() {
        let (drum, other, plain) = (
            taker(1, &[(1, &[9])]),
            taker(2, &[(1, &[9])]),
            taker(3, &[]),
        );
        // A plan that uses every source it is given.
        let planner = |chosen: &[ManaOption]| {
            Some(Payment {
                taps: chosen.iter().map(|option| option.source).collect(),
                ..Payment::default()
            })
        };
        let better = |a: &Payment, b: &Payment| a.taps.len() > b.taps.len();
        let found = solving(&[], &[drum, other, plain.clone()], &planner, &better).unwrap();
        assert_eq!(
            found.taps.len(),
            2,
            "one of the two that share a permanent is left out"
        );
        assert!(found.taps.contains(&plain.source));
        let found = solving(&[], std::slice::from_ref(&plain), &planner, &better).unwrap();
        assert_eq!(found.taps, [plain.source]);
    }
}
