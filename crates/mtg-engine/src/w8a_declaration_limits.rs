//! Limits on a whole declaration (CR 508.1c, 509.1b): "No more than one creature can attack each combat." and "... can
//! block each combat." A declaration with more creatures than the least limit in force is illegal, and with a limit the
//! requirements are obeyed as far as it lets them (CR 508.1d, 509.1c): a creature that must attack or block may be left
//! out only by a declaration that is full of creatures that must.

use mtg_core::ids::ObjectId;

use crate::combat::AttackTarget;
use crate::decision::{AttackOptions, BlockOptions};
use crate::error::IllegalAnswer;

/// `named` cut to `most` entries: those whose creature `required` names first, then the others, each in its order.
fn w8a_cut<T: Copy>(
    named: &[(ObjectId, T)],
    required: &[ObjectId],
    most: Option<usize>,
) -> Vec<(ObjectId, T)> {
    let Some(most) = most.filter(|most| named.len() > *most) else {
        return named.to_vec();
    };
    let is_required = |object: &ObjectId| required.contains(object);
    let kept: Vec<ObjectId> = named
        .iter()
        .filter(|(object, _)| is_required(object))
        .chain(named.iter().filter(|(object, _)| !is_required(object)))
        .map(|(object, _)| *object)
        .take(most)
        .collect();
    named
        .iter()
        .copied()
        .filter(|(object, _)| kept.contains(object))
        .collect()
}

/// Refuses a declaration of more creatures than `most`, and says whether the limit excuses a creature that must and is
/// left out: the declaration is full and every creature in it must too.
pub(crate) fn w8a_check_limit(
    most: Option<usize>,
    required: &[ObjectId],
    declared: &[ObjectId],
) -> Result<bool, IllegalAnswer> {
    let Some(most) = most else {
        return Ok(false);
    };
    if declared.len() > most {
        return Err(IllegalAnswer::TooManyCreatures {
            allowed: most,
            got: declared.len(),
        });
    }
    Ok(declared.len() == most && declared.iter().all(|object| required.contains(object)))
}

impl AttackOptions {
    /// `attack` cut to the limit on the declaration, the creatures that must attack kept first; unchanged without one.
    pub fn w8a_capped(&self, attack: &[(ObjectId, AttackTarget)]) -> Vec<(ObjectId, AttackTarget)> {
        w8a_cut(attack, &self.required, self.most)
    }
}

impl BlockOptions {
    /// `blocks` cut to the limit on the declaration, the creatures that must block kept first, without an attacker
    /// left with fewer blockers than it needs (menace); unchanged without a limit.
    pub fn w8a_capped(&self, blocks: &[(ObjectId, ObjectId)]) -> Vec<(ObjectId, ObjectId)> {
        let mut cut = w8a_cut(blocks, &self.required, self.most);
        if cut.len() < blocks.len() {
            let short: Vec<ObjectId> = cut
                .iter()
                .map(|(_, attacker)| *attacker)
                .filter(|attacker| {
                    cut.iter().filter(|(_, held)| held == attacker).count()
                        < self.needed_for(*attacker)
                })
                .collect();
            cut.retain(|(_, attacker)| !short.contains(attacker));
        }
        cut
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a cut keeps the creatures that must first and the order of the declaration, never leaves a menaced
    //! attacker short, and the check refuses a declaration over the limit and excuses a missing creature only when the
    //! declaration is full of creatures that must.
    use rstest::rstest;

    use super::*;

    fn ids(raw: &[u64]) -> Vec<ObjectId> {
        raw.iter().copied().map(ObjectId).collect()
    }

    #[rstest]
    #[case::no_limit(None, &[1, 2, 3], &[3], &[1, 2, 3])]
    #[case::under_the_limit(Some(3), &[1, 2], &[], &[1, 2])]
    #[case::required_first(Some(1), &[1, 2, 3], &[3], &[3])]
    #[case::order_kept(Some(2), &[1, 2, 3], &[3], &[1, 3])]
    #[case::free_ones_fill(Some(2), &[1, 2, 3], &[], &[1, 2])]
    fn w8a_cut_keeps_what_must(
        #[case] most: Option<usize>,
        #[case] named: &[u64],
        #[case] required: &[u64],
        #[case] kept: &[u64],
    ) {
        let named: Vec<(ObjectId, ())> = ids(named).into_iter().map(|id| (id, ())).collect();
        let cut: Vec<ObjectId> = w8a_cut(&named, &ids(required), most)
            .into_iter()
            .map(|(id, ())| id)
            .collect();
        assert_eq!(cut, ids(kept));
    }

    #[rstest]
    #[case::no_limit(None, &[1], &[1, 2, 3], Ok(false))]
    #[case::over(Some(1), &[], &[1, 2], Err(IllegalAnswer::TooManyCreatures { allowed: 1, got: 2 }))]
    #[case::full_of_required(Some(1), &[1, 2], &[2], Ok(true))]
    #[case::full_with_a_free_one(Some(1), &[1], &[2], Ok(false))]
    #[case::room_left(Some(2), &[1, 2], &[2], Ok(false))]
    fn w8a_check_refuses_and_excuses(
        #[case] most: Option<usize>,
        #[case] required: &[u64],
        #[case] declared: &[u64],
        #[case] expected: Result<bool, IllegalAnswer>,
    ) {
        assert_eq!(
            w8a_check_limit(most, &ids(required), &ids(declared)),
            expected
        );
    }

    #[test]
    fn w8a_a_menaced_attacker_is_never_left_short() {
        let options = BlockOptions {
            blockers: Vec::new(),
            min_blockers: vec![(ObjectId(9), 2)],
            max_blockers: Vec::new(),
            required: Vec::new(),
            most: Some(2),
        };
        let blocks = [
            (ObjectId(1), ObjectId(8)),
            (ObjectId(2), ObjectId(9)),
            (ObjectId(3), ObjectId(9)),
        ];
        assert_eq!(
            options.w8a_capped(&blocks),
            vec![(ObjectId(1), ObjectId(8))]
        );
    }
}
