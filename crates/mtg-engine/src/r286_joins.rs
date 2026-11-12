//! Join rules of the attack and block declarations (CR 508.1c, 509.1b): a creature that can attack or block only beside
//! enough other creatures of a kind ("can't attack unless at least two other creatures attack", "can't block unless a
//! creature with greater power also blocks"), and one that can attack only alone. The options carry each rule with the
//! creatures that may meet it, and the check refuses a declaration that breaks one.

use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::ObjectId;
use serde::{Deserialize, Serialize};

use crate::error::IllegalAnswer;
use crate::resolve::Scope;
use crate::state::GameState;

/// A creature of the declaration that takes part only beside others, or only alone.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct JoinRule {
    /// The creature the rule binds.
    pub object: ObjectId,
    /// How many of `among` must take part beside it.
    pub at_least: u32,
    /// The other creatures of the options that count.
    pub among: Vec<ObjectId>,
    /// It takes part only alone: no other creature may.
    pub alone: bool,
}

/// The join rules of the creatures `offered` (the attackers or the blockers of the options), with `attack` choosing the
/// attack rules or the block rules; `free` are the attackers that can attack for nothing, which "all creatures you control
/// attack if able" binds.
pub(crate) fn r286_joins(
    state: &GameState,
    offered: &[ObjectId],
    free: &[ObjectId],
    attack: bool,
) -> Vec<JoinRule> {
    let mut rules = Vec::new();
    for object in offered {
        let Some(held) = state.object(*object) else {
            continue;
        };
        let controller = crate::characteristics::controller_of_object(state, held);
        let scope = Scope::announcing(state, controller, held.reference());
        for quirk in state.rules().quirks.of(*object) {
            let (others, among) = match quirk {
                CombatQuirk::AttackAlone if attack => {
                    rules.push(JoinRule {
                        object: *object,
                        at_least: 0,
                        among: Vec::new(),
                        alone: true,
                    });
                    continue;
                }
                CombatQuirk::AttackWithAll if attack => {
                    let among: Vec<ObjectId> = free
                        .iter()
                        .copied()
                        .filter(|other| other != object)
                        .collect();
                    rules.push(JoinRule {
                        object: *object,
                        at_least: u32::try_from(among.len()).unwrap_or(u32::MAX),
                        among,
                        alone: false,
                    });
                    continue;
                }
                CombatQuirk::AttackWithOthers { others, among } if attack => (others, among),
                CombatQuirk::BlockWithOthers { others, among } if !attack => (others, among),
                _ => continue,
            };
            let among = offered
                .iter()
                .copied()
                .filter(|other| other != object)
                .filter(|other| {
                    state.object(*other).is_some_and(|candidate| {
                        scope.selects(among, candidate.reference()).unwrap_or(false)
                    })
                })
                .collect();
            rules.push(JoinRule {
                object: *object,
                at_least: *others,
                among,
                alone: false,
            });
        }
    }
    rules
}

/// Whether a join rule binds the creature, so no requirement forces it to take part alone (CR 508.1d).
pub(crate) fn r286_bound(rules: &[JoinRule], object: ObjectId) -> bool {
    rules.iter().any(|rule| rule.object == object)
}

/// Refuses a declaration of `declared` (each creature once) that breaks a join rule; `what` names the creatures.
pub(crate) fn r286_check(
    rules: &[JoinRule],
    declared: &[ObjectId],
    what: &'static str,
) -> Result<(), IllegalAnswer> {
    for rule in rules.iter().filter(|rule| declared.contains(&rule.object)) {
        if rule.alone && declared.len() > 1 {
            return Err(IllegalAnswer::OnlyAlone(what));
        }
        let joined = rule
            .among
            .iter()
            .filter(|other| declared.contains(other))
            .count();
        if joined < usize::try_from(rule.at_least).unwrap_or(usize::MAX) {
            return Err(IllegalAnswer::TooFewBeside(what));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Goal: the check refuses exactly the declarations that leave a bound creature short of its others or not alone.
    use super::*;
    use rstest::rstest;

    fn id(n: u64) -> ObjectId {
        ObjectId(n)
    }

    fn rule(at_least: u32, among: &[u64], alone: bool) -> JoinRule {
        JoinRule {
            object: id(1),
            at_least,
            among: among.iter().copied().map(id).collect(),
            alone,
        }
    }

    #[rstest]
    #[case::two_of_three(rule(2, &[2, 3, 4], false), &[1, 2, 3], true)]
    #[case::one_of_two_needed(rule(2, &[2, 3, 4], false), &[1, 2], false)]
    #[case::outside_the_kind(rule(1, &[2], false), &[1, 3], false)]
    #[case::of_the_kind(rule(1, &[2], false), &[1, 2], true)]
    #[case::not_declared(rule(1, &[2], false), &[3], true)]
    #[case::alone(rule(0, &[], true), &[1], true)]
    #[case::not_alone(rule(0, &[], true), &[1, 2], false)]
    #[case::others_without_it(rule(0, &[], true), &[2, 3], true)]
    fn a_declaration_meets_the_join_rules(
        #[case] held: JoinRule,
        #[case] declared: &[u64],
        #[case] legal: bool,
    ) {
        let declared: Vec<ObjectId> = declared.iter().copied().map(id).collect();
        assert_eq!(r286_check(&[held], &declared, "attacker").is_ok(), legal);
    }
}
