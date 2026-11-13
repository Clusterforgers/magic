//! "If a creature you control attacks, this creature also attacks if able." (`CombatQuirk::AttackIfAnyAttacks`, CR
//! 508.1d): a creature bound this way that can attack for nothing must join any declaration naming a creature its
//! selector picks. Each such pair is a join rule on the other creature ("if it attacks, one of these attacks"), added
//! after the plain requirements so it frees no creature from them.

use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::ObjectId;

use crate::r286_joins::JoinRule;
use crate::resolve::Scope;
use crate::state::GameState;

/// The join rules the creatures `offered` with `AttackIfAnyAttacks` make: for each such creature in `free` and each other
/// offered creature its selector picks (judged for the bound creature's controller), that creature attacks only beside it.
pub(crate) fn r400_follow_joins(
    state: &GameState,
    offered: &[ObjectId],
    free: &[ObjectId],
) -> Vec<JoinRule> {
    let mut rules = Vec::new();
    for bound in free {
        let Some(held) = state.object(*bound) else {
            continue;
        };
        let controller = crate::characteristics::controller_of_object(state, held);
        let scope = Scope::announcing(state, controller, held.reference());
        for quirk in state.rules().quirks.of(*bound) {
            let CombatQuirk::AttackIfAnyAttacks(among) = quirk else {
                continue;
            };
            for other in offered.iter().filter(|other| *other != bound) {
                let picked = state.object(*other).is_some_and(|candidate| {
                    scope.selects(among, candidate.reference()).unwrap_or(false)
                });
                if picked {
                    rules.push(JoinRule {
                        object: *other,
                        at_least: 1,
                        among: vec![*bound],
                        alone: false,
                    });
                }
            }
        }
    }
    rules
}
