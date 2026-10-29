//! Creatures that block several attackers (CR 509.1a): "can block an additional creature each combat". A blocker is
//! offered with how many attackers it may block beyond the first, the declaration names it once for each attacker it
//! blocks, and its combat damage is divided among those attackers (CR 510.1d).

use std::collections::BTreeMap;

use mtg_core::ids::ObjectId;

use crate::combat::Attacker;
use crate::decision::BlockOptions;
use crate::error::IllegalAnswer;
use crate::effects::Modification;
use crate::state::GameState;
use crate::zones::Zone;

/// How many attackers beyond the first the creature `id` may block, as the effects that apply to it say: the sum of every
/// `BlocksAdditional` (each "can block an additional creature" adds one).
pub(crate) fn w14c_extra_blocks(state: &GameState, id: ObjectId) -> usize {
    state
        .object(id)
        .filter(|object| object.zone() == Zone::Battlefield)
        .and_then(|object| state.layers().get(object.id()))
        .map_or(0, |held| {
            held.applied
                .iter()
                .map(|applied| match applied.modification {
                    Modification::BlocksAdditional(count) => count as usize,
                    _ => 0,
                })
                .sum()
        })
}

/// Checks the pairs of a block declaration: a blocker is named at most once for each attacker and for no more
/// attackers than its options allow.
pub(crate) fn w14c_check_pairs(
    options: &BlockOptions,
    answer: &[(ObjectId, ObjectId)],
) -> Result<(), IllegalAnswer> {
    for (i, (blocker, attacker)) in answer.iter().enumerate() {
        if answer[..i].contains(&(*blocker, *attacker)) {
            return Err(IllegalAnswer::Duplicate("blocker"));
        }
        let allowed = options
            .blockers
            .iter()
            .find(|b| b.object == *blocker)
            .ok_or(IllegalAnswer::NotOffered("blocker"))?;
        if !allowed.can_block.contains(attacker) {
            return Err(IllegalAnswer::NotOffered("attacker to block"));
        }
        let named = answer[..=i].iter().filter(|(b, _)| b == blocker).count();
        if named > 1 + allowed.extra {
            return Err(IllegalAnswer::Duplicate("blocker"));
        }
    }
    Ok(())
}

/// The distinct blockers of a declaration, in the order first named: what a limit on the creatures that block counts.
pub(crate) fn w14c_distinct_blockers(answer: &[(ObjectId, ObjectId)]) -> Vec<ObjectId> {
    let mut blockers = Vec::with_capacity(answer.len());
    for (blocker, _) in answer {
        if !blockers.contains(blocker) {
            blockers.push(*blocker);
        }
    }
    blockers
}

/// The share of combat damage each blocker that blocks several attackers deals to each of them, keyed by (blocker,
/// attacker): lethal damage to each attacker in the order they attack and the rest to the last (CR 510.1d). A blocker
/// that blocks one attacker is left out: it deals all its damage to it.
pub(crate) fn w14c_blocker_shares(
    state: &GameState,
    attackers: &[Attacker],
    power: impl Fn(ObjectId) -> Option<u32>,
) -> BTreeMap<(ObjectId, ObjectId), u32> {
    let mut blocked: BTreeMap<ObjectId, Vec<ObjectId>> = BTreeMap::new();
    for attacker in attackers {
        for blocker in attacker.blockers() {
            blocked.entry(*blocker).or_default().push(attacker.object());
        }
    }
    let mut shares = BTreeMap::new();
    for (blocker, attacked) in blocked.into_iter().filter(|(_, a)| a.len() > 1) {
        let Some(mut left) = power(blocker) else {
            continue;
        };
        let deathtouch = crate::keywords::damage_rules(state, blocker).deathtouch;
        for (at, attacker) in attacked.iter().enumerate() {
            let last = at + 1 == attacked.len();
            let lethal = crate::damage::lethal_damage(state, *attacker, deathtouch);
            let share = if last { left } else { left.min(lethal) };
            left -= share;
            shares.insert((blocker, *attacker), share);
        }
    }
    shares
}
