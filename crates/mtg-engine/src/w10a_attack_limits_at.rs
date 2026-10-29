//! Limits on the attacks at one player or planeswalker (CR 508.1c): "No more than two creatures can attack you each
//! combat." and "No more than one creature can attack ~ each combat." A declaration with more creatures at a limited
//! defender than its limit is illegal, and a creature that must attack may be left out only when each defender it may
//! attack for nothing is full of creatures that must (CR 508.1d).

use mtg_core::ids::ObjectId;

use crate::combat::AttackTarget;
use crate::decision::{AttackOptions, AttackerOption};
use crate::error::IllegalAnswer;

/// The limit on the attacks at `target` among `most_at`, if one limits it.
fn w10a_limit_of(most_at: &[(AttackTarget, usize)], target: AttackTarget) -> Option<usize> {
    most_at
        .iter()
        .find(|(limited, _)| *limited == target)
        .map(|(_, most)| *most)
}

/// The creatures of `attack` that attack `target`.
fn w10a_at(
    attack: &[(ObjectId, AttackTarget)],
    target: AttackTarget,
) -> impl Iterator<Item = ObjectId> + '_ {
    attack
        .iter()
        .filter(move |(_, at)| *at == target)
        .map(|(object, _)| *object)
}

/// Whether `option` may attack `target` for nothing: the creature may attack it and no tax asks mana or life for it.
fn w10a_free_at(option: &AttackerOption, target: AttackTarget) -> bool {
    option.defenders.contains(&target)
        && !option.tax.iter().any(|(taxed, _)| *taxed == target)
        && !option.life.iter().any(|(taxed, _)| *taxed == target)
}

/// Refuses a declaration with more creatures at a limited player or planeswalker than its limit.
pub(crate) fn w10a_check_limits_at(
    most_at: &[(AttackTarget, usize)],
    attack: &[(ObjectId, AttackTarget)],
) -> Result<(), IllegalAnswer> {
    for (target, most) in most_at {
        let got = w10a_at(attack, *target).count();
        if got > *most {
            return Err(IllegalAnswer::TooManyCreatures {
                allowed: *most,
                got,
            });
        }
    }
    Ok(())
}

impl AttackOptions {
    /// Whether the limits at the defenders excuse `object`, a creature that must attack and is left out of `attack`:
    /// each defender it may attack for nothing has a limit, filled with creatures that must attack.
    pub(crate) fn w10a_excused_at(
        &self,
        object: ObjectId,
        attack: &[(ObjectId, AttackTarget)],
    ) -> bool {
        let Some(option) = self.attackers.iter().find(|option| option.object == object) else {
            return false;
        };
        let full_of_required = |target: AttackTarget| {
            w10a_limit_of(&self.most_at, target).is_some_and(|most| {
                let at: Vec<ObjectId> = w10a_at(attack, target).collect();
                at.len() == most && at.iter().all(|object| self.required.contains(object))
            })
        };
        option
            .defenders
            .iter()
            .copied()
            .filter(|target| w10a_free_at(option, *target))
            .all(full_of_required)
    }

    /// `attack` within the limits at the defenders: the creatures that must attack placed first, each where it was
    /// declared while that defender has room, else at the first defender with room it may attack for nothing, else
    /// left out; the order of the declaration is kept. Unchanged without a limit at a defender.
    pub fn w10a_within_limits_at(
        &self,
        attack: &[(ObjectId, AttackTarget)],
    ) -> Vec<(ObjectId, AttackTarget)> {
        if self.most_at.is_empty() {
            return attack.to_vec();
        }
        let is_required = |object: &ObjectId| self.required.contains(object);
        let mut placed: Vec<(ObjectId, AttackTarget)> = Vec::new();
        let ordered = attack
            .iter()
            .filter(|(object, _)| is_required(object))
            .chain(attack.iter().filter(|(object, _)| !is_required(object)));
        for (object, target) in ordered {
            let room = |at: AttackTarget, placed: &[(ObjectId, AttackTarget)]| {
                w10a_limit_of(&self.most_at, at)
                    .is_none_or(|most| w10a_at(placed, at).count() < most)
            };
            let moved = || {
                let option = self
                    .attackers
                    .iter()
                    .find(|option| option.object == *object)?;
                option
                    .defenders
                    .iter()
                    .copied()
                    .find(|at| w10a_free_at(option, *at) && room(*at, &placed))
            };
            let at = if room(*target, &placed) {
                Some(*target)
            } else {
                moved()
            };
            if let Some(at) = at {
                placed.push((*object, at));
            }
        }
        attack
            .iter()
            .filter_map(|(object, _)| placed.iter().find(|(kept, _)| kept == object).copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the attacks at a limited defender never exceed its limit; a creature over it moves to a free defender with
    //! room or is left out, the creatures that must attack placed first; and a creature that must attack is excused
    //! only when each defender it may attack for nothing is full of creatures that must.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;

    const P1: AttackTarget = AttackTarget::Player(PlayerId(1));
    const P2: AttackTarget = AttackTarget::Player(PlayerId(2));

    /// Creatures 1 to 4 that may attack players 1 and 2 for nothing, `required` of them must, player 1 limited.
    fn w10a_options(required: &[u64], most: usize) -> AttackOptions {
        AttackOptions {
            attackers: (1..=4)
                .map(|id| AttackerOption::free(ObjectId(id), vec![P1, P2]))
                .collect(),
            required: required.iter().copied().map(ObjectId).collect(),
            mana: 0,
            most: None,
            most_at: vec![(P1, most)],
        }
    }

    fn w10a_attack(raw: &[(u64, AttackTarget)]) -> Vec<(ObjectId, AttackTarget)> {
        raw.iter().map(|(id, at)| (ObjectId(*id), *at)).collect()
    }

    #[rstest]
    #[case::within(1, &[], &[(1, P1), (2, P2)], &[(1, P1), (2, P2)])]
    #[case::moved(1, &[], &[(1, P1), (2, P1)], &[(1, P1), (2, P2)])]
    #[case::required_first(1, &[2], &[(1, P1), (2, P1)], &[(1, P2), (2, P1)])]
    #[case::two_fit(2, &[], &[(1, P1), (2, P1), (3, P1)], &[(1, P1), (2, P1), (3, P2)])]
    fn w10a_within_limits_places_what_must_first(
        #[case] most: usize,
        #[case] required: &[u64],
        #[case] declared: &[(u64, AttackTarget)],
        #[case] kept: &[(u64, AttackTarget)],
    ) {
        let options = w10a_options(required, most);
        let placed = options.w10a_within_limits_at(&w10a_attack(declared));
        assert_eq!(placed, w10a_attack(kept));
        assert_eq!(w10a_check_limits_at(&options.most_at, &placed), Ok(()));
    }

    #[test]
    fn w10a_a_creature_with_no_room_anywhere_is_left_out() {
        let mut options = w10a_options(&[], 1);
        options.attackers = vec![
            AttackerOption::free(ObjectId(1), vec![P1]),
            AttackerOption::free(ObjectId(2), vec![P1]),
        ];
        let placed = options.w10a_within_limits_at(&w10a_attack(&[(1, P1), (2, P1)]));
        assert_eq!(placed, w10a_attack(&[(1, P1)]));
    }

    #[rstest]
    #[case::over(&[(1, P1), (2, P1)], Err(IllegalAnswer::TooManyCreatures { allowed: 1, got: 2 }))]
    #[case::at_the_limit(&[(1, P1), (2, P2), (3, P2)], Ok(()))]
    fn w10a_the_check_refuses_more_than_the_limit(
        #[case] declared: &[(u64, AttackTarget)],
        #[case] expected: Result<(), IllegalAnswer>,
    ) {
        let options = w10a_options(&[], 1);
        assert_eq!(
            w10a_check_limits_at(&options.most_at, &w10a_attack(declared)),
            expected
        );
    }

    #[rstest]
    #[case::room_elsewhere(&[P1, P2], &[1, 2], &[(1, P1)], false)]
    #[case::full_of_required(&[P1], &[1, 2], &[(1, P1)], true)]
    #[case::full_of_free(&[P1], &[2], &[(1, P1)], false)]
    #[case::room_at_the_limit(&[P1], &[2], &[], false)]
    fn w10a_excused_only_by_defenders_full_of_required(
        #[case] defenders: &[AttackTarget],
        #[case] required: &[u64],
        #[case] declared: &[(u64, AttackTarget)],
        #[case] excused: bool,
    ) {
        let mut options = w10a_options(required, 1);
        options.attackers = (1..=2)
            .map(|id| AttackerOption::free(ObjectId(id), defenders.to_vec()))
            .collect();
        assert_eq!(
            options.w10a_excused_at(ObjectId(2), &w10a_attack(declared)),
            excused
        );
    }
}
