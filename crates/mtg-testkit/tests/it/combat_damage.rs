//! Goal: combat damage, blocking and defender choice follow the rules for every attacker/blocker mix, including several blockers and many players.

use crate::common;

use common::combat::{duel, survivors, to_blockers};
use common::{on_battlefield, pid};
use mtg_engine::decision::Answer;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameResult};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
#[rstest]
#[case::trade((2, 2), (2, 2), false, false)]
#[case::attacker_wins((3, 3), (1, 1), true, false)]
#[case::blocker_wins((1, 1), (3, 3), false, true)]
#[case::both_bounce((2, 3), (2, 3), true, true)]
#[case::big_trade((4, 4), (4, 4), false, false)]
#[case::zero_power_attacker((0, 1), (2, 2), false, true)]
#[case::zero_power_blocker((5, 1), (0, 5), true, false)]
#[case::wall_stops_attack((1, 5), (1, 5), true, true)]
#[case::exact_lethal_one_way((3, 2), (1, 3), true, false)]
#[case::small_trade((2, 1), (1, 1), false, false)]
fn one_blocker_one_attacker(
    #[case] attacker: (i32, i32),
    #[case] blocker: (i32, i32),
    #[case] attacker_lives: bool,
    #[case] blocker_lives: bool,
) -> Result<(), ScriptError> {
    let mut driver = duel(attacker, blocker)?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    driver.idle_until(2, Step::EndCombat)?;
    let mut expected = Vec::new();
    if attacker_lives {
        expected.push("A".to_owned());
    }
    if blocker_lives {
        expected.push("B".to_owned());
    }
    assert_eq!(survivors(&driver), expected);
    driver.expect_life(1, 20)?;
    driver.expect_life(0, 20)
}

#[rstest]
#[case::nothing(0, 20)]
#[case::one(1, 19)]
#[case::five(5, 15)]
#[case::exactly_lethal(20, 0)]
#[case::overkill(25, -5)]
fn unblocked_attackers_hit_the_player(
    #[case] power: i32,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A", power, 3))])
            .battlefield(1, [ready(free_creature("B", 1, 1))]),
    )?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[])?;
    if life > 0 {
        driver.idle_until(2, Step::EndCombat)?;
        driver.expect_life(1, life)?;
        driver.expect_no_event("zero damage", |e| {
            matches!(e, Event::DamageDealt { amount: 0, .. })
        })?;
    } else {
        driver.idle_until(2, Step::EndCombat).ok();
        assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(0))));
        assert_eq!(
            driver
                .state()
                .player(pid(1))
                .map(mtg_engine::player::Player::life),
            Some(life)
        );
    }
    Ok(())
}

type Blocker = (&'static str, i32, i32);

#[rstest]
#[case::split_kills_two((5, 5), vec![("B1", 2, 2), ("B2", 2, 2), ("B3", 3, 3)], vec![2, 2, 1], vec!["B3"])]
#[case::all_on_first((4, 4), vec![("B1", 3, 3), ("B2", 3, 3)], vec![4, 0], vec!["B2"])]
#[case::all_on_second((4, 9), vec![("B1", 1, 1), ("B2", 1, 4)], vec![0, 4], vec!["A", "B1"])]
#[case::attacker_survives_three((2, 9), vec![("B1", 1, 1), ("B2", 1, 1), ("B3", 1, 1)], vec![1, 1, 0], vec!["A", "B3"])]
#[case::damage_spread_too_thin((3, 9), vec![("B1", 1, 2), ("B2", 1, 2), ("B3", 1, 2)], vec![1, 1, 1], vec!["A", "B1", "B2", "B3"])]
fn several_blockers_split_the_attackers_damage(
    #[case] attacker: (i32, i32),
    #[case] blockers: Vec<Blocker>,
    #[case] split: Vec<u32>,
    #[case] alive: Vec<&str>,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A", attacker.0, attacker.1))])
            .battlefield(
                1,
                blockers
                    .iter()
                    .map(|(n, p, t)| ready(free_creature(n, *p, *t))),
            ),
    )?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    let blocks: Vec<(&str, &str)> = blockers.iter().map(|(n, _, _)| (*n, "A")).collect();
    driver.block(&blocks)?;
    // Both players pass the declare-blockers round; then the attacker's controller divides damage.
    driver.pass()?;
    driver.pass()?;
    driver.expect_decision(0, "assign-damage")?;
    let shares: Vec<(&str, u32)> = blockers.iter().map(|(n, _, _)| *n).zip(split).collect();
    driver.assign_damage(&shares)?;
    driver.idle_until(2, Step::EndCombat)?;
    let mut expected: Vec<String> = alive.iter().map(|s| (*s).to_owned()).collect();
    expected.sort();
    assert_eq!(survivors(&driver), expected);
    Ok(())
}

#[rstest]
#[case::too_little(vec![("B1", 1), ("B2", 0)])]
#[case::too_much(vec![("B1", 3), ("B2", 1)])]
#[case::strangers(vec![("A", 4)])]
#[case::same_blocker_twice(vec![("B1", 2), ("B1", 2)])]
fn invalid_damage_divisions_are_rejected(
    #[case] shares: Vec<(&str, u32)>,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A", 3, 3))])
            .battlefield(
                1,
                [
                    ready(free_creature("B1", 1, 1)),
                    ready(free_creature("B2", 1, 1)),
                ],
            ),
    )?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("B1", "A"), ("B2", "A")])?;
    driver.pass()?;
    driver.pass()?;
    let before = driver.engine().state_hash();
    let answer: Vec<_> = shares
        .iter()
        .filter_map(|(n, amount)| Some((on_battlefield(driver.state(), n)?, *amount)))
        .collect();
    assert!(driver.answer(Answer::AssignDamage(answer)).is_err());
    assert_eq!(driver.engine().state_hash(), before);
    driver.expect_decision(0, "assign-damage")
}

#[test]
fn a_blocked_attacker_deals_no_damage_to_the_player() -> Result<(), ScriptError> {
    let mut driver = duel((4, 4), (0, 6))?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 20)
}

#[test]
fn two_attackers_one_blocker_blocks_one_and_the_other_connects() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(free_creature("Big", 3, 3)),
                    ready(free_creature("Small", 1, 1)),
                ],
            )
            .battlefield(1, [ready(free_creature("Wall", 0, 4))]),
    )?;
    driver.attack(&[("Big", 1), ("Small", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("Wall", "Big")])?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 19)
}

#[test]
fn damage_is_marked_apart_from_toughness_and_wears_off_at_cleanup() -> Result<(), ScriptError> {
    let mut driver = duel((1, 1), (1, 3))?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    driver.idle_until(2, Step::EndCombat)?;
    let blocker = on_battlefield(driver.state(), "B")
        .ok_or_else(|| ScriptError::Expectation("B died".into()))?;
    let marked = driver
        .state()
        .object(blocker)
        .and_then(|o| o.permanent())
        .map(mtg_engine::object::PermanentState::damage);
    assert_eq!(marked, Some(1));
    let chars = mtg_engine::characteristics(driver.state(), blocker);
    assert_eq!(
        chars.and_then(|c| c.toughness),
        Some(3),
        "toughness itself is untouched"
    );
    driver.idle_until(3, Step::Upkeep)?;
    let marked = driver
        .state()
        .object(blocker)
        .and_then(|o| o.permanent())
        .map(mtg_engine::object::PermanentState::damage);
    assert_eq!(marked, Some(0));
    driver.expect_event("DamageCleared", |e| matches!(e, Event::DamageCleared))
}
