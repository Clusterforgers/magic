//! Mana abilities a payment uses more than once (Cadaverous Bloom exiles a card for each use, Skirk Prospector sacrifices
//! a Goblin for each).
//!
//! The planners treat a permanent as one source of a payment. A source with `ManaOption::repeat` is therefore copied for
//! the length of one call: use `n` of a permanent is the source of an object `n` strides above it, and `mana.rs` plans,
//! checks and completes over the copies as over distinct permanents. What leaves a public function is folded back: the
//! copies become the permanent, which a payment names once for each use. A permanent that also taps for mana is one
//! source with its other abilities for its first use, as a permanent always was.

use std::borrow::Cow;

use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;

use crate::error::PaymentError;
use crate::mana::{ManaOption, ManaSource, Payment};
use crate::spend::Spent;

/// The most times one ability is used in a payment.
const MOST: usize = 8;

/// The distance between the numbers of the copies of a permanent, far above any object of a game.
const STRIDE: u64 = 1 << 40;

/// The permanent `object` stands for: itself, or the permanent it is a copy of.
pub(crate) const fn real(object: ObjectId) -> ObjectId {
    ObjectId(object.0 % STRIDE)
}

/// The object whose source is the `nth` use (counting from 0) of the permanent `object`.
fn copy(object: ObjectId, nth: usize) -> ObjectId {
    ObjectId(object.0 + STRIDE * u64::try_from(nth).unwrap_or(0))
}

/// How many times a payment of a cost worth `value` can use `option`: no more than what its cost takes allows, and
/// no more than `value`, since a payment has nothing to spare and so has no more parts than the cost has mana.
fn uses(option: &ManaOption, value: u32) -> usize {
    let takes = option
        .takes
        .iter()
        .map(|take| take.from.len() / take.count.max(1) as usize)
        .min()
        .unwrap_or(0);
    takes
        .min(usize::try_from(value).unwrap_or(usize::MAX))
        .min(MOST)
}

/// `sources` with a copy for each further use of the ones a payment of `cost` can use again.
pub(crate) fn expand<'a>(sources: &'a [ManaOption], cost: &ManaCost) -> Cow<'a, [ManaOption]> {
    if !sources.iter().any(|option| option.repeat) {
        return Cow::Borrowed(sources);
    }
    let value = cost.mana_value();
    let mut all = sources.to_vec();
    for option in sources.iter().filter(|option| option.repeat) {
        for nth in 1..uses(option, value) {
            let mut again = option.clone();
            again.source.object = copy(option.source.object, nth);
            all.push(again);
        }
    }
    Cow::Owned(all)
}

/// The tap of `payment` that names a copy: only the engine numbers those, so an answer that does is refused.
pub(crate) fn forged(payment: &Payment) -> Result<(), PaymentError> {
    match payment.taps.iter().find(|tap| real(tap.object) != tap.object) {
        Some(tap) => Err(PaymentError::UnknownSource(*tap)),
        None => Ok(()),
    }
}

/// `payment` with the second and later uses of a permanent named as the copies `sources` list, so no source is named
/// twice. A use beyond the copies keeps the permanent's own name, which the check refuses as a source named twice.
pub(crate) fn unfold(payment: &Payment, sources: &[ManaOption]) -> Payment {
    let mut unfolded = payment.clone();
    let mut used: Vec<ObjectId> = Vec::new();
    for tap in &mut unfolded.taps {
        if !sources
            .iter()
            .any(|option| option.repeat && option.source == *tap)
        {
            continue;
        }
        let nth = used.iter().filter(|object| **object == tap.object).count();
        used.push(tap.object);
        let again = ManaSource {
            object: copy(tap.object, nth),
            ability: tap.ability,
        };
        if sources.iter().any(|option| option.source == again) {
            *tap = again;
        }
    }
    unfolded
}

/// `payment` with each copy named as the permanent it is a copy of.
pub(crate) fn fold(mut payment: Payment) -> Payment {
    for tap in &mut payment.taps {
        tap.object = real(tap.object);
    }
    payment
}

/// `spent` with each copy named as the permanent it is a copy of.
pub(crate) fn fold_spent(mut spent: Spent) -> Spent {
    for step in &mut spent.steps {
        step.source.object = real(step.source.object);
    }
    spent
}

#[cfg(test)]
mod tests {
    //! Goal: a source that may be used again gets one copy for each use its cost can take and its mana can still use, a payment that names a permanent more than once is read as its copies and goes back to the permanent's name, and a copy nobody offered is refused.
    use mtg_core::mana::ManaType;
    use rstest::rstest;

    use super::*;
    use crate::mana::Take;
    use crate::spend::Step;

    /// A source of the permanent `object` that exiles `count` of the `from` cards for `made` mana of one type.
    fn bloom(object: u64, ability: usize, made: u32, from: &[u64], repeat: bool) -> ManaOption {
        ManaOption {
            count: made,
            repeat,
            takes: vec![Take {
                sacrifice: false,
                exile: true,
                count: 1,
                from: from.iter().map(|id| ObjectId(*id)).collect(),
            }],
            ..ManaOption::new(
                ManaSource {
                    object: ObjectId(object),
                    ability,
                },
                ManaType::Black,
            )
        }
    }

    fn cost(text: &str) -> ManaCost {
        ManaCost::parse(text).unwrap()
    }

    #[rstest]
    #[case::one_card_one_use("{B}{B}{B}{B}", &[10], 1)]
    #[case::two_cards_two_uses("{B}{B}{B}{B}", &[10, 11], 2)]
    #[case::the_cost_has_four_mana("{B}{B}{B}{B}", &[10, 11, 12, 13, 14], 4)]
    #[case::a_cost_of_three("{B}{B}{B}", &[10, 11, 12, 13], 3)]
    #[case::no_cards_no_use("{B}{B}", &[], 0)]
    #[case::a_cost_of_one("{B}", &[10, 11], 1)]
    #[case::at_most_eight("{20}", &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 8)]
    fn a_source_has_a_copy_for_every_use_it_can_have(
        #[case] cost_text: &str,
        #[case] from: &[u64],
        #[case] copies: usize,
    ) {
        let sources = [bloom(1, 0, 2, from, true)];
        let all = expand(&sources, &cost(cost_text));
        let copies_of = all.iter().filter(|o| real(o.source.object) == ObjectId(1));
        assert_eq!(copies_of.count(), copies.max(1));
        let mut objects: Vec<_> = all.iter().map(|o| o.source.object).collect();
        objects.sort();
        objects.dedup();
        assert_eq!(objects.len(), all.len(), "every copy is a distinct object");
    }

    #[test]
    fn a_source_used_once_is_not_copied() {
        let sources = [bloom(1, 0, 2, &[10, 11], false)];
        assert!(matches!(
            expand(&sources, &cost("{B}{B}{B}{B}")),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn every_ability_of_a_permanent_has_its_own_copies() {
        let sources = [
            bloom(1, 0, 2, &[10, 11], true),
            bloom(1, 1, 2, &[10, 11], true),
        ];
        let all = expand(&sources, &cost("{B}{B}{B}{B}"));
        assert_eq!(all.len(), 4, "two abilities, two uses of each");
        let second: Vec<_> = all
            .iter()
            .filter(|o| o.source.object == copy(ObjectId(1), 1))
            .map(|o| o.source.ability)
            .collect();
        assert_eq!(second, [0, 1]);
    }

    #[rstest]
    #[case::once(&[(1, 0)], &[(1, 0)])]
    #[case::twice_the_same_ability(&[(1, 0), (1, 0)], &[(1, 0), (2, 0)])]
    #[case::two_abilities(&[(1, 0), (1, 1)], &[(1, 0), (2, 1)])]
    #[case::beside_another_permanent(&[(1, 0), (5, 0), (1, 0)], &[(1, 0), (5, 0), (2, 0)])]
    #[case::three_uses_are_not_offered(&[(1, 0), (1, 0), (1, 0)], &[(1, 0), (2, 0), (1, 0)])]
    fn a_payment_names_the_uses_of_a_permanent_as_its_copies(
        #[case] named: &[(u64, usize)],
        #[case] read: &[(u64, usize)],
    ) {
        // Permanent 1 has two copies; the copy of it is told as object 2 below.
        let source = |(object, ability): (u64, usize)| ManaSource {
            object: if object == 2 {
                copy(ObjectId(1), 1)
            } else {
                ObjectId(object)
            },
            ability,
        };
        let offered = [
            bloom(1, 0, 2, &[10, 11], true),
            bloom(1, 1, 2, &[10, 11], true),
            ManaOption::new(source((5, 0)), ManaType::Green),
        ];
        let sources = expand(&offered, &cost("{B}{B}{B}{B}"));
        let payment = Payment {
            taps: named.iter().copied().map(source).collect(),
            ..Payment::default()
        };
        let unfolded = unfold(&payment, &sources);
        let expected: Vec<ManaSource> = read.iter().copied().map(source).collect();
        assert_eq!(unfolded.taps, expected);
        if expected.iter().all(|tap| {
            sources
                .iter()
                .any(|o| o.source == *tap)
        }) {
            assert_eq!(fold(unfolded).taps, payment.taps, "back to the permanents");
        }
    }

    #[test]
    fn a_copy_nobody_offered_is_refused() {
        let copied = ManaSource {
            object: copy(ObjectId(1), 1),
            ability: 0,
        };
        let payment = Payment {
            taps: vec![copied],
            ..Payment::default()
        };
        assert_eq!(forged(&payment), Err(PaymentError::UnknownSource(copied)));
        let plain = Payment {
            taps: vec![ManaSource {
                object: ObjectId(1),
                ability: 0,
            }],
            ..Payment::default()
        };
        assert_eq!(forged(&plain), Ok(()));
    }

    #[test]
    fn the_steps_of_a_payment_name_the_permanent() {
        let spent = Spent {
            steps: vec![Step {
                source: ManaSource {
                    object: copy(ObjectId(3), 2),
                    ability: 1,
                },
                paid: Vec::new(),
            }],
            ..Spent::default()
        };
        assert_eq!(fold_spent(spent).steps[0].source.object, ObjectId(3));
    }
}
