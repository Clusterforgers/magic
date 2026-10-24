//! Goal: a pool keeps mana for the time its lifetime says (the step, the combat phase, the turn), spends the mana that goes soonest first, empties by the lifetime asked for, counts what an emptying takes, and two pools that hold the same mana are equal however they got there.

use mtg_cards::spend::{ManaLasts, SpendOn};
use mtg_core::mana::ManaType;
use rstest::rstest;

use crate::mana::ManaPool;

use ManaLasts::{Combat, Step, Turn};
use ManaType::{Green, Red};

/// A pool after these additions, each `(mana, lasts)` without a restriction.
fn pool(added: &[(ManaType, ManaLasts)]) -> ManaPool {
    let mut pool = ManaPool::default();
    for (mana, lasts) in added {
        pool.add_restricted(*mana, Vec::new(), *lasts);
    }
    pool
}

fn kept(pool: &ManaPool, mana: ManaType) -> (u32, u32) {
    pool.kept().map_or((0, 0), |kept| {
        (kept.combat_end[mana.index()], kept.turn_end[mana.index()])
    })
}

#[rstest]
#[case::the_step_takes_only_the_mana_of_the_step(Step, 2, 2, (1, 1))]
#[case::a_phase_takes_the_combat_mana_too(Combat, 3, 1, (0, 1))]
#[case::the_turn_takes_everything(Turn, 4, 0, (0, 0))]
fn emptying_takes_the_mana_that_lasts_no_longer_than_the_lifetime(
    #[case] through: ManaLasts,
    #[case] taken: u32,
    #[case] red_left: u32,
    #[case] red_kept: (u32, u32),
) {
    let mut held = pool(&[(Red, Step), (Red, Combat), (Red, Turn), (Green, Step)]);
    assert_eq!(held.emptying(through), taken);
    held.empty(through);
    assert_eq!(held.total(), 4 - taken);
    assert_eq!(held.get(Red), red_left);
    assert_eq!(kept(&held, Red), red_kept);
}

#[rstest]
#[case::the_step_mana_first(&[(Red, Turn), (Red, Step)], 1, (0, 1))]
#[case::combat_mana_before_turn_mana(&[(Red, Turn), (Red, Combat)], 1, (0, 1))]
#[case::the_step_mana_then_the_combat_mana(&[(Red, Turn), (Red, Combat), (Red, Step)], 2, (0, 1))]
#[case::all_of_it(&[(Red, Turn), (Red, Combat), (Red, Step)], 3, (0, 0))]
fn the_mana_that_goes_soonest_is_spent_first(
    #[case] added: &[(ManaType, ManaLasts)],
    #[case] spent: usize,
    #[case] left: (u32, u32),
) {
    let mut held = pool(added);
    for _ in 0..spent {
        assert!(held.spend(Red));
    }
    assert_eq!(kept(&held, Red), left);
    assert_eq!(held.get(Red), u32::try_from(added.len() - spent).unwrap());
}

#[test]
fn a_pool_that_kept_nothing_is_equal_to_one_that_never_did() {
    let mut held = pool(&[(Red, Turn)]);
    assert!(held.spend(Red));
    assert_eq!(held, ManaPool::default());
    let mut emptied = pool(&[(Green, Combat), (Red, Step)]);
    emptied.empty(Combat);
    assert_eq!(emptied, ManaPool::default());
}

#[rstest]
#[case::the_step(Step, 2, (1, 1))]
#[case::a_phase(Combat, 1, (0, 1))]
#[case::the_turn(Turn, 0, (0, 0))]
fn restricted_mana_goes_by_its_lifetime_too(
    #[case] through: ManaLasts,
    #[case] units: usize,
    #[case] green: (u32, u32),
) {
    let only = vec![SpendOn::abilities()];
    let mut held = ManaPool::default();
    held.add_restricted(Red, only.clone(), Step);
    held.add_restricted(Red, only.clone(), Combat);
    held.add_restricted(Red, only, Turn);
    held.add_restricted(Green, Vec::new(), Combat);
    held.add_restricted(Green, Vec::new(), Turn);
    held.empty(through);
    assert_eq!(held.restricted().len(), units);
    assert!(held.restricted().iter().all(|unit| unit.lasts > through));
    assert_eq!(kept(&held, Green), green);
}

#[rstest]
#[case::only_step_mana_at_the_end_of_a_phase(&[(Red, Step)], Combat, Step)]
#[case::only_step_mana_at_the_end_of_the_turn(&[(Red, Step)], Turn, Step)]
#[case::combat_mana_at_the_end_of_a_phase(&[(Red, Step), (Red, Combat)], Combat, Combat)]
#[case::combat_mana_at_the_end_of_the_turn(&[(Red, Combat)], Turn, Combat)]
#[case::turn_mana_at_the_end_of_the_turn(&[(Red, Combat), (Red, Turn)], Turn, Turn)]
#[case::turn_mana_kept_at_the_end_of_a_phase(&[(Red, Turn)], Combat, Step)]
fn the_log_names_the_shortest_lifetime_that_empties_the_same_mana(
    #[case] added: &[(ManaType, ManaLasts)],
    #[case] through: ManaLasts,
    #[case] named: ManaLasts,
) {
    let held = pool(added);
    assert_eq!(held.least_through(through), named);
    let (mut by_named, mut by_through) = (held.clone(), held);
    by_named.empty(named);
    by_through.empty(through);
    assert_eq!(by_named, by_through);
}
