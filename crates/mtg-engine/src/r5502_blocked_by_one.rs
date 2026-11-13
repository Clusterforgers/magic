//! "Equipped creature ... must be blocked by an Eldrazi if able." (`CombatQuirk::MustBeBlockedByOne`, CR 509.1c): a
//! requirement on the block declaration that only a creature the selector picks (judged for the attacker's controller)
//! meets. Each attacker with it is listed with the offered blockers that could meet it; the declaration has one of them
//! block it unless none could without breaking a rule or giving up a requirement of its own.

use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::ObjectId;

use crate::decision::{BlockOptions, BlockerOption};
use crate::resolve::Scope;
use crate::state::GameState;

/// Each attacker some offered blocker could block that must be blocked by a creature of a kind, with the offered
/// blockers of that kind that could block it.
pub(crate) fn r5502_blocked_by_one(
    state: &GameState,
    blockers: &[BlockerOption],
) -> Vec<(ObjectId, Vec<ObjectId>)> {
    let mut attackers: Vec<ObjectId> = Vec::new();
    for option in blockers {
        for attacker in &option.can_block {
            if !attackers.contains(attacker) {
                attackers.push(*attacker);
            }
        }
    }
    let mut found = Vec::new();
    for attacker in attackers {
        let Some(held) = state.object(attacker) else {
            continue;
        };
        let controller = crate::characteristics::controller_of_object(state, held);
        let scope = Scope::announcing(state, controller, held.reference());
        for quirk in state.rules().quirks.of(attacker) {
            let CombatQuirk::MustBeBlockedByOne(kind) = quirk else {
                continue;
            };
            let able: Vec<ObjectId> = blockers
                .iter()
                .filter(|option| option.can_block.contains(&attacker))
                .filter(|option| {
                    state.object(option.object).is_some_and(|blocker| {
                        scope.selects(kind, blocker.reference()).unwrap_or(false)
                    })
                })
                .map(|option| option.object)
                .collect();
            if !able.is_empty() && !found.contains(&(attacker, able.clone())) {
                found.push((attacker, able));
            }
        }
    }
    found
}

impl BlockOptions {
    /// Whether the declaration leaves an attacker listed in `must_be_blocked_by` unblocked by every creature that could
    /// meet it while one of them could still block it.
    pub(crate) fn r5502_unmet(&self, blocks: &[(ObjectId, ObjectId)]) -> bool {
        self.must_be_blocked_by
            .iter()
            .flat_map(|listed| listed.iter())
            .any(|(attacker, able)| {
                !blocks
                    .iter()
                    .any(|(blocker, held)| held == attacker && able.contains(blocker))
                    && self
                        .blockers
                        .iter()
                        .filter(|option| able.contains(&option.object))
                        .any(|option| self.could_serve(option, *attacker, blocks))
            })
    }

    /// Adds to `blocks` a creature of the kind for each listed attacker none of them blocks, if one is free.
    pub(crate) fn r5502_serve(&self, blocks: &mut Vec<(ObjectId, ObjectId)>) {
        for (attacker, able) in self
            .must_be_blocked_by
            .iter()
            .flat_map(|listed| listed.iter())
        {
            if blocks
                .iter()
                .any(|(blocker, held)| held == attacker && able.contains(blocker))
            {
                continue;
            }
            let free = able
                .iter()
                .find(|blocker| !blocks.iter().any(|(held, _)| held == *blocker));
            if let Some(blocker) = free {
                blocks.push((*blocker, *attacker));
            }
        }
    }
}
