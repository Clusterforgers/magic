//! Goal: the automatic answer to an offer of attackers or blockers is always one the check accepts, whatever mix of
//! creatures that must or can't act alone, attackers that must be blocked, limits and menace the offer holds.
use mtg_core::ids::{ObjectId, PlayerId};
use proptest::prelude::*;

use crate::combat::{AttackTarget, check_attackers, check_blockers};
use crate::decision::{AttackOptions, AttackerOption, BlockOptions, BlockerOption};
use crate::legal_declaration::search;

const FOE: AttackTarget = AttackTarget::Player(PlayerId(1));

fn id(raw: u64) -> ObjectId {
    ObjectId(raw)
}

/// An offer built the way the engine builds it: a creature that can't act alone is never required, and the attackers
/// that must be blocked or need two blockers are offered ones.
fn block_offer(
    can: &[Vec<bool>],
    flags: &[(bool, bool)],
    owed: &[bool],
    menace: &[bool],
    most: Option<usize>,
) -> BlockOptions {
    let attackers = owed.len();
    let rows: Vec<(BlockerOption, (bool, bool))> = can
        .iter()
        .zip(flags)
        .enumerate()
        .map(|(i, (row, flag))| {
            let option = BlockerOption {
                object: id(i as u64 + 1),
                can_block: (0..attackers)
                    .filter(|a| row[*a])
                    .map(|a| id(100 + a as u64))
                    .collect(),
                extra: 0,
            };
            (option, *flag)
        })
        .filter(|(option, _)| !option.can_block.is_empty())
        .collect();
    let offered = |attacker: ObjectId| rows.iter().any(|(b, _)| b.can_block.contains(&attacker));
    let cant_alone: Vec<ObjectId> = rows
        .iter()
        .filter(|(_, (alone, _))| *alone)
        .map(|(b, _)| b.object)
        .collect();
    let required = rows
        .iter()
        .filter(|(b, (_, req))| *req && !cant_alone.contains(&b.object))
        .map(|(b, _)| b.object)
        .collect();
    let named = |wanted: &[bool]| -> Vec<ObjectId> {
        (0..attackers)
            .filter(|a| wanted[*a] && offered(id(100 + *a as u64)))
            .map(|a| id(100 + a as u64))
            .collect()
    };
    BlockOptions {
        must_be_blocked: named(owed),
        min_blockers: named(menace)
            .into_iter()
            .filter(|a| {
                // The offer drops an attacker that fewer creatures than it needs could block together.
                most.is_none_or(|most| most >= 2)
                    && rows.iter().filter(|(b, _)| b.can_block.contains(a)).count() >= 2
            })
            .map(|a| (a, 2))
            .collect(),
        max_blockers: Vec::new(),
        required,
        most,
        cant_alone,
        blockers: rows.into_iter().map(|(b, _)| b).collect(),
    }
}

/// Whether any declaration of at most one block per blocker is accepted.
fn some_declaration_is_accepted(options: &BlockOptions) -> bool {
    let slots: Vec<Vec<(ObjectId, ObjectId)>> = options
        .blockers
        .iter()
        .map(|b| b.can_block.iter().map(|a| (b.object, *a)).collect())
        .collect();
    let mut budget = usize::MAX;
    search(&slots, &|blocks| options.accepts(blocks), &mut budget).is_some()
}

proptest! {
    #[test]
    fn the_automatic_blocks_are_always_accepted(
        can in prop::collection::vec(prop::collection::vec(any::<bool>(), 4), 1..6),
        flags in prop::collection::vec((any::<bool>(), any::<bool>()), 6),
        must in prop::collection::vec(any::<bool>(), 4),
        menace in prop::collection::vec(prop::bool::weighted(0.2), 4),
        most in prop::option::of(1usize..4),
    ) {
        let options = block_offer(&can, &flags, &must, &menace, most);
        prop_assert!(some_declaration_is_accepted(&options), "{:?}", options);
        let blocks = options.legal_blocks();
        prop_assert_eq!(check_blockers(&options, &blocks), Ok(()), "{:?} {:?}", options, blocks);
    }

    #[test]
    fn the_automatic_attack_is_always_accepted(
        flags in prop::collection::vec((any::<bool>(), any::<bool>()), 1..7),
        most in prop::option::of(1usize..4),
    ) {
        let attackers: Vec<AttackerOption> = (0..flags.len())
            .map(|i| AttackerOption::free(id(i as u64 + 1), vec![FOE]))
            .collect();
        let cant_alone: Vec<ObjectId> = attackers
            .iter()
            .zip(&flags)
            .filter(|(_, (alone, _))| *alone)
            .map(|(a, _)| a.object)
            .collect();
        let required = attackers
            .iter()
            .zip(&flags)
            .filter(|(a, (_, req))| *req && !cant_alone.contains(&a.object))
            .map(|(a, _)| a.object)
            .collect();
        let options = AttackOptions {
            attackers,
            required,
            mana: 0,
            most,
            most_at: Vec::new(),
            cant_alone,
        };
        let attack = options.legal_attack();
        prop_assert_eq!(check_attackers(&options, &attack), Ok(()), "{:?} {:?}", options, attack);
    }
}

#[test]
fn an_attacker_that_must_be_blocked_brings_a_second_blocker_to_one_that_cant_block_alone() {
    let blocker = |object, can| BlockerOption {
        object: id(object),
        can_block: vec![id(can)],
        extra: 0,
    };
    let options = BlockOptions {
        blockers: vec![blocker(1, 100), blocker(2, 100)],
        min_blockers: Vec::new(),
        max_blockers: Vec::new(),
        required: Vec::new(),
        must_be_blocked: vec![id(100)],
        most: None,
        cant_alone: vec![id(1)],
    };
    let blocks = options.legal_blocks();
    assert!(options.accepts(&blocks) && !blocks.is_empty(), "{blocks:?}");
}
