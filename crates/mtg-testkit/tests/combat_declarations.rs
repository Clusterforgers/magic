//! Goal: players choose attackers and defenders freely among many opponents, are told when a decision is skipped, and illegal declarations are rejected untouched.

mod common;

use common::combat::{duel, missing_if_none, survivors, to_blockers};
use common::{on_battlefield, pid};
use mtg_engine::Event;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

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
    assert_eq!(
        options.attackers[0].defenders.len(),
        players - 1,
        "every opponent is a legal defender"
    );
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
    driver.expect_no_event(
        "blockers asked from bystanders",
        |e| matches!(e, Event::BlockersDeclared { player, .. } if *player != pid(target)),
    )
}

#[test]
fn each_defender_may_only_block_attackers_aimed_at_them() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(free_creature("ToOne", 2, 2)),
                    ready(free_creature("ToTwo", 2, 2)),
                ],
            )
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
    assert_eq!(
        Some(options.blockers[0].can_block.clone()),
        one.map(|o| vec![o])
    );
    let two = on_battlefield(driver.state(), "ToTwo");
    let before = driver.engine().state_hash();
    let block_wrong = driver.answer(Answer::DeclareBlockers(vec![(
        options.blockers[0].object,
        missing_if_none(two),
    )]));
    assert!(block_wrong.is_err());
    assert_eq!(driver.engine().state_hash(), before);
    driver.block(&[("D1", "ToOne")])?;
    driver.expect_decision(2, "declare-blockers")?;
    driver.block(&[("D2", "ToTwo")])?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(
        survivors(&driver),
        ["ToOne", "ToTwo"].map(String::from).to_vec(),
        "the 1/1 blockers died"
    );
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
        .map(mtg_engine::object::PermanentState::tapped);
    assert_eq!(tapped, Some(true));
    to_blockers(&mut driver, 1)?;
    driver.expect_decision(1, "declare-blockers")
}

#[test]
fn choosing_not_to_attack_skips_blocks_and_damage_without_a_nothing_to_do()
-> Result<(), ScriptError> {
    let mut driver = duel((3, 3), (1, 1))?;
    driver.attack(&[])?;
    driver.expect_step(Step::EndCombat)?;
    driver.expect_no_event("NothingToDo", |e| matches!(e, Event::NothingToDo { .. }))?;
    driver.expect_no_event("blockers step", |e| {
        matches!(
            e,
            Event::StepBegan {
                step: Step::DeclareBlockers
            }
        )
    })
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
        return driver
            .expect_no_event("any attack", |e| matches!(e, Event::PermanentTapped { .. }));
    }
    driver.attack(&[("A", 1)])?;
    for _ in 0..2 {
        if driver
            .engine()
            .events()
            .iter()
            .any(|e| matches!(e, Event::NothingToDo { .. }))
        {
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
fn illegal_attack_declarations_are_rejected_untouched(
    #[case] target: AttackTarget,
) -> Result<(), ScriptError> {
    let mut driver = duel((2, 2), (2, 2))?;
    let mine = missing_if_none(on_battlefield(driver.state(), "A"));
    let theirs = missing_if_none(on_battlefield(driver.state(), "B"));
    let before = driver.engine().state_hash();
    let attempts = [
        vec![(mine, target)],
        vec![(theirs, AttackTarget::Player(pid(1)))],
        vec![
            (mine, AttackTarget::Player(pid(1))),
            (mine, AttackTarget::Player(pid(1))),
        ],
    ];
    for attempt in attempts {
        assert!(driver.answer(Answer::DeclareAttackers(attempt)).is_err());
        assert_eq!(driver.engine().state_hash(), before);
    }
    driver.expect_decision(0, "declare-attackers")
}
