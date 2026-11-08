//! The declaration of attackers or blockers an automatic answer gives: one the check accepts, found by the same check that
//! validates a player's declaration, so offer, default and validation never disagree.

use mtg_core::ids::ObjectId;

use crate::combat::{AttackTarget, check_attackers, check_blockers};
use crate::decision::{AttackOptions, BlockOptions};

/// The most complete declarations the search tries before it gives up and returns the plain default.
const SEARCH_LIMIT: usize = 50_000;

/// Walks the declarations that give each slot one of its choices (the first choice is "none"), in that order, and returns
/// the first `accepted` takes.
pub(crate) fn search<T: Copy>(
    slots: &[Vec<T>],
    accepted: &dyn Fn(&[T]) -> bool,
    budget: &mut usize,
) -> Option<Vec<T>> {
    fn walk<T: Copy>(
        slots: &[Vec<T>],
        picked: &mut Vec<T>,
        accepted: &dyn Fn(&[T]) -> bool,
        budget: &mut usize,
    ) -> bool {
        let Some((slot, rest)) = slots.split_first() else {
            *budget = budget.saturating_sub(1);
            return accepted(picked);
        };
        if *budget == 0 {
            return false;
        }
        if walk(rest, picked, accepted, budget) {
            return true;
        }
        for choice in slot {
            picked.push(*choice);
            if walk(rest, picked, accepted, budget) {
                return true;
            }
            picked.pop();
        }
        false
    }
    let mut picked = Vec::new();
    walk(slots, &mut picked, accepted, budget).then_some(picked)
}

impl AttackOptions {
    /// Whether the check accepts the declaration.
    pub fn accepts(&self, attack: &[(ObjectId, AttackTarget)]) -> bool {
        check_attackers(self, attack).is_ok()
    }

    /// A declaration the check accepts, doing as little as the rules allow: what the rules require, else the fewest
    /// attackers that make it legal (a creature that can't attack alone brings another). The plain requirement when
    /// no search result exists.
    pub fn legal_attack(&self) -> Vec<(ObjectId, AttackTarget)> {
        let plain = self.required_attack();
        if self.accepts(&plain) {
            return plain;
        }
        let slots: Vec<Vec<(ObjectId, AttackTarget)>> = self
            .attackers
            .iter()
            .map(|option| {
                option
                    .free_defender()
                    .map(|target| (option.object, target))
                    .into_iter()
                    .collect()
            })
            .collect();
        let accepted = |attack: &[(ObjectId, AttackTarget)]| self.accepts(attack);
        let mut budget = SEARCH_LIMIT;
        search(&slots, &accepted, &mut budget).unwrap_or(plain)
    }
}

impl BlockOptions {
    /// Whether the check accepts the declaration.
    pub fn accepts(&self, blocks: &[(ObjectId, ObjectId)]) -> bool {
        check_blockers(self, blocks).is_ok()
    }

    /// A declaration the check accepts, doing as little as the rules allow: what the rules require, else the fewest
    /// blocks that meet the most requirements (CR 509.1c). The plain requirement when no search result exists.
    pub fn legal_blocks(&self) -> Vec<(ObjectId, ObjectId)> {
        let plain = self.required_blocks();
        if self.accepts(&plain) {
            return plain;
        }
        // The creatures a rule binds come first so the search settles them before the free ones.
        let mut blockers: Vec<_> = self.blockers.iter().collect();
        blockers.sort_by_key(|option| {
            !(self.required.contains(&option.object)
                || option
                    .can_block
                    .iter()
                    .any(|attacker| self.must_be_blocked.contains(attacker)))
        });
        let slots: Vec<Vec<(ObjectId, ObjectId)>> = blockers
            .into_iter()
            .map(|option| {
                option
                    .can_block
                    .iter()
                    .map(|attacker| (option.object, *attacker))
                    .collect()
            })
            .collect();
        let accepted = |blocks: &[(ObjectId, ObjectId)]| self.accepts(blocks);
        let mut budget = SEARCH_LIMIT;
        search(&slots, &accepted, &mut budget).unwrap_or(plain)
    }
}
