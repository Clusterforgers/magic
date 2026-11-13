//! "~ must be blocked by two or more creatures if able." (`CombatQuirk::R6001MustBeBlockedByAtLeast`, CR 509.1c): a
//! requirement on the block declaration. Each attacker with it is listed with the number; the declaration has that many
//! creatures block it unless fewer could, without breaking a rule or giving up a requirement of their own.

use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::ObjectId;

use crate::decision::{BlockOptions, BlockerOption};
use crate::state::GameState;

/// Each attacker some offered blocker could block that must be blocked by several creatures if able, with how many.
pub(crate) fn r6001_blocked_by_at_least(
    state: &GameState,
    blockers: &[BlockerOption],
) -> Vec<(ObjectId, usize)> {
    let mut found: Vec<(ObjectId, usize)> = Vec::new();
    for attacker in blockers.iter().flat_map(|option| option.can_block.iter()) {
        if found.iter().any(|(held, _)| held == attacker) {
            continue;
        }
        let most = state
            .rules()
            .quirks
            .of(*attacker)
            .filter_map(|quirk| match quirk {
                CombatQuirk::R6001MustBeBlockedByAtLeast(count) => usize::try_from(*count).ok(),
                _ => None,
            })
            .max();
        if let Some(count) = most {
            found.push((*attacker, count));
        }
    }
    found
}

impl BlockOptions {
    /// Whether the declaration leaves an attacker listed in `r6001_at_least` with fewer blockers than it needs while
    /// another creature could still join its block.
    pub(crate) fn r6001_unmet(&self, blocks: &[(ObjectId, ObjectId)]) -> bool {
        self.r6001_at_least.iter().any(|(attacker, count)| {
            let have = blocks.iter().filter(|(_, held)| held == attacker).count();
            have < *count
                && self.blockers.iter().any(|option| {
                    !blocks
                        .iter()
                        .any(|(blocker, held)| *blocker == option.object && held == attacker)
                        && self.could_serve(option, *attacker, blocks)
                })
        })
    }

    /// Adds free creatures to the block of each listed attacker until it has as many as it needs or none is left.
    pub(crate) fn r6001_serve(&self, blocks: &mut Vec<(ObjectId, ObjectId)>) {
        for (attacker, count) in &self.r6001_at_least {
            for option in &self.blockers {
                let have = blocks.iter().filter(|(_, held)| held == attacker).count();
                if have >= *count {
                    break;
                }
                if option.can_block.contains(attacker)
                    && !blocks.iter().any(|(blocker, _)| *blocker == option.object)
                {
                    blocks.push((option.object, *attacker));
                }
            }
        }
    }
}
