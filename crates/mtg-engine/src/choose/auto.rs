//! The plain answers a table gives for a seat that asks it to (`docs/ui-design.md`, Priority, stops and information leaks): its triggers in the order
//! they triggered, and combat damage lethal to each blocker in order. Both read only what every player sees.

use mtg_core::ids::ObjectId;

use super::{TriggersChoice, TriggersOptions};
use crate::decision::DamageOptions;

/// The triggers in the order they triggered, by timestamp: the first goes on the stack first and resolves last.
pub fn triggers_in_order(options: &TriggersOptions) -> TriggersChoice {
    TriggersChoice((0..options.triggers.len()).collect())
}

/// Lethal damage to each blocker in the order offered, the rest to the last one, or to the player attacked when
/// trampling. `None` when there is no blocker to take the damage.
pub fn damage_in_order(options: &DamageOptions) -> Option<Vec<(ObjectId, u32)>> {
    let mut left = options.power;
    let mut shares: Vec<(ObjectId, u32)> = options
        .blockers
        .iter()
        .enumerate()
        .map(|(at, blocker)| {
            let share = options
                .lethal
                .get(at)
                .map_or(left, |lethal| left.min(*lethal));
            left -= share;
            (*blocker, share)
        })
        .collect();
    if options.excess.is_none() {
        shares.last_mut()?.1 += left;
    }
    Some(shares)
}

#[cfg(test)]
mod tests {
    //! Goal: the auto answers are legal for every shape of damage and trigger list: lethal in order with the rest to the last blocker or past them with trample, and triggers in the order they triggered.
    use rstest::rstest;

    use super::*;
    use crate::choose::TriggerEntry;
    use crate::combat::DamageTarget;
    use crate::damage::assignment_answer;
    use crate::object::ObjectRef;
    use mtg_core::ids::PlayerId;

    fn ids(n: u64) -> Vec<ObjectId> {
        (1..=n).map(ObjectId).collect()
    }

    #[rstest]
    #[case::one_blocker_takes_all(5, 1, vec![2], false, Some(vec![5]))]
    #[case::lethal_in_order_rest_to_last(5, 2, vec![2, 2], false, Some(vec![2, 3]))]
    #[case::too_little_for_the_second(3, 2, vec![2, 4], false, Some(vec![2, 1]))]
    #[case::deathtouch_lethal_one(3, 3, vec![1, 1, 1], false, Some(vec![1, 1, 1]))]
    #[case::trample_carries_the_rest(6, 2, vec![2, 1], true, Some(vec![2, 1]))]
    #[case::no_lethal_known_takes_all(4, 1, vec![], false, Some(vec![4]))]
    #[case::no_blocker(4, 0, vec![], false, None)]
    fn damage_goes_lethal_in_order(
        #[case] power: u32,
        #[case] blockers: u64,
        #[case] lethal: Vec<u32>,
        #[case] trample: bool,
        #[case] expected: Option<Vec<u32>>,
    ) {
        let options = DamageOptions {
            attacker: ObjectId(99),
            power,
            blockers: ids(blockers),
            excess: trample.then_some(DamageTarget::Player(PlayerId::from_index(1))),
            lethal,
            unblocked: false,
        };
        let answer = damage_in_order(&options);
        let shares = answer
            .as_ref()
            .map(|a| a.iter().map(|(_, n)| *n).collect::<Vec<_>>());
        assert_eq!(shares, expected);
        if let Some(answer) = answer {
            assert!(assignment_answer(&options, answer).is_ok());
        }
    }

    #[rstest]
    #[case::two(2)]
    #[case::five(5)]
    fn triggers_keep_their_order(#[case] n: usize) {
        let entry = |at: u64| TriggerEntry {
            source: ObjectRef {
                id: ObjectId(at),
                counter: 0,
            },
            ability: 0,
            when: String::new(),
        };
        let options = TriggersOptions {
            triggers: (0..n as u64).map(entry).collect(),
        };
        assert_eq!(
            triggers_in_order(&options),
            TriggersChoice((0..n).collect())
        );
    }
}
