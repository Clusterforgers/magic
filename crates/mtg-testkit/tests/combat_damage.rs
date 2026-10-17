//! Goal: combat damage, blocking and defender choice follow the rules for every attacker/blocker mix, including several blockers and many players.

mod common;

use common::{names, on_battlefield, pid};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameResult};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{ready, Driver, Scenario, ScriptError};
use rstest::rstest;

/// Two players, P0 about to declare attackers with `A`, P1 holding `B`.
fn duel(attacker: (i32, i32), blocker: (i32, i32)) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A", attacker.0, attacker.1))])
            .battlefield(1, [ready(free_creature("B", blocker.0, blocker.1))]),
    )
}

/// Passes priority until the blockers decision (after the declare-attackers priority round).
fn to_blockers(driver: &mut Driver, defenders: usize) -> Result<(), ScriptError> {
    for _ in 0..driver.state().players().len() {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.pass()?;
    }
    let _ = defenders;
    Ok(())
}

/// An id that exists nowhere, so a failed lookup turns into an illegal answer rather than a panic.
fn missing_if_none(id: Option<mtg_core::ids::ObjectId>) -> mtg_core::ids::ObjectId {
    id.unwrap_or(mtg_core::ids::ObjectId(u64::MAX))
}

fn survivors(driver: &Driver) -> Vec<String> {
    let mut alive = names(driver.state(), Zone::Battlefield);
    alive.sort();
    alive
}

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
fn unblocked_attackers_hit_the_player(#[case] power: i32, #[case] life: i32) -> Result<(), ScriptError> {
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
        driver.expect_no_event("zero damage", |e| matches!(e, Event::DamageDealt { amount: 0, .. }))?;
    } else {
        driver.idle_until(2, Step::EndCombat).ok();
        assert_eq!(driver.engine().result(), Some(GameResult::Winner(pid(0))));
        assert_eq!(driver.state().player(pid(1)).map(|p| p.life()), Some(life));
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
            .battlefield(1, blockers.iter().map(|(n, p, t)| ready(free_creature(n, *p, *t)))),
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
fn invalid_damage_divisions_are_rejected(#[case] shares: Vec<(&str, u32)>) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A", 3, 3))])
            .battlefield(1, [ready(free_creature("B1", 1, 1)), ready(free_creature("B2", 1, 1))]),
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
            .battlefield(0, [ready(free_creature("Big", 3, 3)), ready(free_creature("Small", 1, 1))])
            .battlefield(1, [ready(free_creature("Wall", 0, 4))]),
    )?;
    driver.attack(&[("Big", 1), ("Small", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("Wall", "Big")])?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 19)
}

#[rstest]
fn the_attacker_picks_which_opponent_to_attack(
    #[values(3, 4, 6)] players: usize,
    #[values(1, 2)] target: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(players)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("A", 3, 3))]);
    for seat in 1..players {
        scenario = scenario.battlefield(seat, [ready(free_creature(&format!("D{seat}"), 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("expected attackers");
    };
    assert_eq!(options.attackers[0].defenders.len(), players - 1, "every opponent is a legal defender");
    driver.attack(&[("A", target)])?;
    for _ in 0..players {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.pass()?;
    }
    driver.expect_decision(target, "declare-blockers")?;
    driver.block(&[])?;
    driver.idle_until(2, Step::EndCombat)?;
    for seat in 0..players {
        driver.expect_life(seat, if seat == target { 17 } else { 20 })?;
    }
    driver.expect_no_event("blockers asked from bystanders", |e| {
        matches!(e, Event::BlockersDeclared { player, .. } if *player != pid(target))
    })
}

#[test]
fn each_defender_may_only_block_attackers_aimed_at_them() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("ToOne", 2, 2)), ready(free_creature("ToTwo", 2, 2))])
            .battlefield(1, [ready(free_creature("D1", 1, 1))])
            .battlefield(2, [ready(free_creature("D2", 1, 1))]),
    )?;
    driver.attack(&[("ToOne", 1), ("ToTwo", 2)])?;
    for _ in 0..3 {
        driver.pass()?;
    }
    driver.expect_decision(1, "declare-blockers")?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("expected blockers");
    };
    let one = on_battlefield(driver.state(), "ToOne");
    assert_eq!(options.blockers.len(), 1);
    assert_eq!(Some(options.blockers[0].can_block.clone()), one.map(|o| vec![o]));
    let two = on_battlefield(driver.state(), "ToTwo");
    let before = driver.engine().state_hash();
    let block_wrong = driver.answer(Answer::DeclareBlockers(vec![(options.blockers[0].object, missing_if_none(two))]));
    assert!(block_wrong.is_err());
    assert_eq!(driver.engine().state_hash(), before);
    driver.block(&[("D1", "ToOne")])?;
    driver.expect_decision(2, "declare-blockers")?;
    driver.block(&[("D2", "ToTwo")])?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(survivors(&driver), ["ToOne", "ToTwo"].map(String::from).to_vec(), "the 1/1 blockers died");
    Ok(())
}

#[test]
fn attackers_tap_and_summoning_sick_creatures_can_still_block() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A", 1, 1))])
            .battlefield(1, [mtg_testkit::sick(free_creature("NewGuy", 1, 1))]),
    )?;
    driver.attack(&[("A", 1)])?;
    let attacker = on_battlefield(driver.state(), "A");
    let tapped = attacker
        .and_then(|id| driver.state().object(id))
        .and_then(|o| o.permanent())
        .map(|p| p.tapped());
    assert_eq!(tapped, Some(true));
    to_blockers(&mut driver, 1)?;
    driver.expect_decision(1, "declare-blockers")
}

#[test]
fn damage_is_marked_apart_from_toughness_and_wears_off_at_cleanup() -> Result<(), ScriptError> {
    let mut driver = duel((1, 1), (1, 3))?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    driver.idle_until(2, Step::EndCombat)?;
    let blocker = on_battlefield(driver.state(), "B").ok_or_else(|| ScriptError::Expectation("B died".into()))?;
    let marked = driver.state().object(blocker).and_then(|o| o.permanent()).map(|p| p.damage());
    assert_eq!(marked, Some(1));
    let chars = mtg_engine::characteristics(driver.state(), blocker);
    assert_eq!(chars.and_then(|c| c.toughness), Some(3), "toughness itself is untouched");
    driver.idle_until(3, Step::Upkeep)?;
    let marked = driver.state().object(blocker).and_then(|o| o.permanent()).map(|p| p.damage());
    assert_eq!(marked, Some(0));
    driver.expect_event("DamageCleared", |e| matches!(e, Event::DamageCleared))
}

#[test]
fn choosing_not_to_attack_skips_blocks_and_damage_without_a_nothing_to_do() -> Result<(), ScriptError> {
    let mut driver = duel((3, 3), (1, 1))?;
    driver.attack(&[])?;
    driver.expect_step(Step::EndCombat)?;
    driver.expect_no_event("NothingToDo", |e| matches!(e, Event::NothingToDo { .. }))?;
    driver.expect_no_event("blockers step", |e| matches!(e, Event::StepBegan { step: Step::DeclareBlockers }))
}

#[rstest]
#[case::no_creatures_at_all(vec![], vec![], true, false)]
#[case::only_sick_creatures(vec![("S", true, false)], vec![], true, false)]
#[case::only_tapped_creatures(vec![("T", false, true)], vec![], true, false)]
#[case::defender_without_creatures(vec![("A", false, false)], vec![], false, true)]
#[case::defender_with_only_tapped_creatures(vec![("A", false, false)], vec![("D", false, true)], false, true)]
fn skipped_decisions_are_announced(
    #[case] attackers: Vec<(&str, bool, bool)>,
    #[case] defenders: Vec<(&str, bool, bool)>,
    #[case] no_attack: bool,
    #[case] no_block: bool,
) -> Result<(), ScriptError> {
    let make = |(name, sick_flag, tapped_flag): (&str, bool, bool)| {
        let card = free_creature(name, 1, 1);
        if sick_flag {
            mtg_testkit::sick(card)
        } else if tapped_flag {
            mtg_testkit::tapped(card)
        } else {
            ready(card)
        }
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, attackers.iter().copied().map(make))
            .battlefield(1, defenders.iter().copied().map(make)),
    )?;
    if no_attack {
        driver.expect_event("NothingToDo(Attack)", |e| {
            matches!(e, Event::NothingToDo { player, kind: mtg_engine::events::NothingToDo::Attack } if *player == pid(0))
        })?;
        return driver.expect_no_event("any attack", |e| matches!(e, Event::PermanentTapped { .. }));
    }
    driver.attack(&[("A", 1)])?;
    for _ in 0..2 {
        if driver.engine().events().iter().any(|e| matches!(e, Event::NothingToDo { .. })) {
            break;
        }
        driver.pass()?;
    }
    assert!(no_block);
    driver.expect_event("NothingToDo(Block)", |e| {
        matches!(e, Event::NothingToDo { player, kind: mtg_engine::events::NothingToDo::Block } if *player == pid(1))
    })?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 19)
}

#[rstest]
#[case::attack_self(AttackTarget::Player(pid(0)))]
#[case::attack_nobody(AttackTarget::Player(pid(7)))]
#[case::attack_missing_walker(AttackTarget::Planeswalker(mtg_core::ids::ObjectId(777)))]
fn illegal_attack_declarations_are_rejected_untouched(#[case] target: AttackTarget) -> Result<(), ScriptError> {
    let mut driver = duel((2, 2), (2, 2))?;
    let mine = missing_if_none(on_battlefield(driver.state(), "A"));
    let theirs = missing_if_none(on_battlefield(driver.state(), "B"));
    let before = driver.engine().state_hash();
    let attempts = [
        vec![(mine, target)],
        vec![(theirs, AttackTarget::Player(pid(1)))],
        vec![(mine, AttackTarget::Player(pid(1))), (mine, AttackTarget::Player(pid(1)))],
    ];
    for attempt in attempts {
        assert!(driver.answer(Answer::DeclareAttackers(attempt)).is_err());
        assert_eq!(driver.engine().state_hash(), before);
    }
    driver.expect_decision(0, "declare-attackers")
}
