//! "~ must be blocked by two or more creatures if able." (`CombatQuirk::R6001MustBeBlockedByAtLeast`, CR 509.1c): a
//! requirement on the block declaration. Each attacker with it is listed in `BlockOptions::must_be_blocked` as many
//! times as it needs blockers; the declaration has that many creatures block it unless fewer could, without breaking a
//! rule or giving up a requirement of their own.

use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::ObjectId;

use crate::decision::{BlockOptions, BlockerOption};
use crate::state::GameState;

/// Lists each attacker some offered blocker could block that must be blocked by several creatures as many times in
/// `listed` as it needs blockers.
pub(crate) fn r6001_list_several(
    state: &GameState,
    blockers: &[BlockerOption],
    listed: &mut Vec<ObjectId>,
) {
    let mut seen: Vec<ObjectId> = Vec::new();
    for attacker in blockers.iter().flat_map(|option| option.can_block.iter()) {
        if seen.contains(attacker) {
            continue;
        }
        seen.push(*attacker);
        let most = state
            .rules()
            .quirks
            .of(*attacker)
            .iter()
            .filter_map(|quirk| match quirk {
                CombatQuirk::R6001MustBeBlockedByAtLeast(count) => usize::try_from(*count).ok(),
                _ => None,
            })
            .max();
        if let Some(count) = most {
            while listed.iter().filter(|held| *held == attacker).count() < count {
                listed.push(*attacker);
            }
        }
    }
}

impl BlockOptions {
    /// Each attacker listed in `must_be_blocked` more than once, with how many times.
    fn r6001_several(&self) -> Vec<(ObjectId, usize)> {
        let mut found: Vec<(ObjectId, usize)> = Vec::new();
        for attacker in &self.must_be_blocked {
            match found.iter_mut().find(|(held, _)| held == attacker) {
                Some((_, count)) => *count += 1,
                None => found.push((*attacker, 1)),
            }
        }
        found.retain(|(_, count)| *count > 1);
        found
    }

    /// Whether the declaration leaves an attacker that must be blocked by several creatures with fewer blockers than it
    /// needs while another creature could still join its block.
    pub(crate) fn r6001_unmet(&self, blocks: &[(ObjectId, ObjectId)]) -> bool {
        self.r6001_several().iter().any(|(attacker, count)| {
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

    /// Adds free creatures to the block of each attacker that must be blocked by several until it has as many as it
    /// needs or none is left.
    pub(crate) fn r6001_serve(&self, blocks: &mut Vec<(ObjectId, ObjectId)>) {
        for (attacker, count) in self.r6001_several() {
            for option in &self.blockers {
                let have = blocks.iter().filter(|(_, held)| *held == attacker).count();
                if have >= count {
                    break;
                }
                if option.can_block.contains(&attacker)
                    && !blocks.iter().any(|(blocker, _)| *blocker == option.object)
                {
                    blocks.push((option.object, attacker));
                }
            }
        }
    }
}
