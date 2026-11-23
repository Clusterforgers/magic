//! Goal: a required additional cost that sacrifices a permanent chosen by X on a spell with no {X} ("As an additional cost to cast this spell, sacrifice an artifact or creature with mana value X.", Nahiri's Sacrifice) asks X as the spell is cast, offers only the values a permanent the caster can sacrifice has, sacrifices one of that mana value and resolves with that X, at tables of two to four seats; with nothing to sacrifice the spell is not cast.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, DivideChoice, NumberChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 with Nahiri's Sacrifice, two Mountains and creatures of mana value 2 and 4; seat 1 with two 2/2 Bears.
fn table(seats: usize, own: bool) -> Scenario {
    let mut own_board = vec![ready(mountain()), ready(mountain())];
    if own {
        own_board.push(ready(creature("Two", 1, &[Color::Green], 1, 1)));
        own_board.push(ready(creature("Four", 3, &[Color::Green], 3, 3)));
    }
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Nahiri's Sacrifice").unwrap()])
        .battlefield(0, own_board)
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Cub", 2, 2)),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// The values of X offered: min, max and the values left out between.
type Offered = (u32, Option<u32>, Vec<u32>);

/// Casts Nahiri's Sacrifice with X = `x`, both Bears its targets and the damage split evenly; what X offered.
fn cast(driver: &mut Driver, x: u32) -> Result<Option<Offered>, ScriptError> {
    driver.begin_cast("Nahiri's Sacrifice")?;
    let mut offered = None;
    for _ in 0..12 {
        let Ok(decision) = driver.decision() else {
            break;
        };
        match &decision.kind.clone() {
            DecisionKind::ChooseNumber(options) => {
                offered = Some((options.min, options.max, options.except.clone()));
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Bear")?;
                let cub = driver.target_named("Cub")?;
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
                    bear, cub,
                ])))?;
            }
            DecisionKind::DivideAmount(options) => {
                let half = options.total / 2;
                let split = vec![
                    (options.among[0], options.total - half),
                    (options.among[1], half),
                ];
                driver.answer(Answer::DivideAmount(DivideChoice::Division(split)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let first = options.from[..1].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) => break,
            other => panic!("unexpected question {}", other.name()),
        }
    }
    Ok(offered)
}

#[rstest]
#[case::two_seats(2, 2)]
#[case::three_seats(3, 4)]
#[case::four_seats(4, 2)]
fn the_sacrifice_takes_a_permanent_of_mana_value_x(
    #[case] seats: usize,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, true))?;
    let offered = cast(&mut driver, x)?;
    // Only a creature of mana value 2 or 4 can be sacrificed: 3 is no value of X.
    assert_eq!(offered, Some((2, Some(4), vec![3])));
    let (gone, kept) = if x == 2 {
        ("Two", "Four")
    } else {
        ("Four", "Two")
    };
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert!(graveyard.contains(&gone.to_owned()), "{graveyard:?}");
    assert!(names(driver.state(), Zone::Battlefield).contains(&kept.to_owned()));
    for _ in 0..20 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    // X damage split between the Bears: 2 kills neither, 4 kills both.
    let dead = names(driver.state(), Zone::Graveyard(pid(1)));
    assert_eq!(dead.len(), if x == 4 { 2 } else { 0 }, "{dead:?}");
    if x == 2 {
        let bear = driver.id_of("Bear")?;
        let permanent = driver.state().object(bear).unwrap().permanent().unwrap();
        assert_eq!(permanent.damage(), 1);
    }
    assert!(
        names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Nahiri's Sacrifice".to_owned())
    );
    Ok(())
}

#[test]
fn without_an_artifact_or_creature_the_sacrifice_is_not_cast() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, false))?;
    assert!(driver.begin_cast("Nahiri's Sacrifice").is_err());
    Ok(())
}
