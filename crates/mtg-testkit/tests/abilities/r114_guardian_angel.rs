//! Goal: Guardian Angel at 2 to 4 seats, aimed at a creature or a player: its spell prevents the next X damage and lets
//! its caster pay {1} any time they have priority until end of turn, as often as they like, each payment a special action
//! that does not use the stack and prevents the next 1 damage to that same permanent or player (`GameRule::R114PayToDo`,
//! CR 116.2, 611.2c); no other seat is offered it, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction, R114PaidDo};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, pinger, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until the stack is empty and the player with priority could act.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The payments offered to the player with priority.
fn payable(driver: &Driver) -> Vec<R114PaidDo> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.r114_payable.clone(),
        _ => Vec::new(),
    }
}

#[rstest]
fn r114_each_payment_prevents_one_more_to_the_same_target(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] payments: usize,
    #[values(false, true)] at_player: bool,
) -> Result<(), ScriptError> {
    let lands: Vec<_> = (0..2 + payments).map(|_| ready(plains())).collect();
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Guardian Angel").unwrap()])
        .battlefield(
            0,
            [
                ready(pinger("Pinger A", 0)),
                ready(pinger("Pinger B", 0)),
                ready(pinger("Pinger C", 0)),
            ]
            .into_iter()
            .chain(lands),
        )
        .battlefield(1, [ready(free_creature("Wall", 0, 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = if at_player {
        driver.target_player(1)
    } else {
        driver.target_named("Wall")?
    };
    driver.begin_cast("Guardian Angel")?;
    for _ in 0..10 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(1)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(std::slice::from_ref(&target))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    settle(&mut driver)?;
    for paid in 0..payments {
        let offers = payable(&driver);
        assert_eq!(offers.len(), 1, "offered again after {paid} payments");
        driver.answer(Answer::Priority(PriorityAction::R114PayToDo(
            offers[0].effect,
        )))?;
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        }
        assert!(
            driver.state().stack().is_empty(),
            "a special action uses no stack"
        );
    }
    // Seat 1 is not offered the caster's payments.
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    assert!(payable(&driver).is_empty());
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    for pinger in ["Pinger A", "Pinger B", "Pinger C"] {
        driver.begin_activation(pinger)?;
        driver.choose_targets(std::slice::from_ref(&target))?;
        settle(&mut driver)?;
    }
    let dealt = 3 - (1 + payments);
    if at_player {
        assert_eq!(
            driver.state().player(pid(1)).unwrap().life(),
            20 - i32::try_from(dealt).unwrap()
        );
    } else {
        assert_eq!(
            names(driver.state(), Zone::Battlefield).contains(&"Wall".to_owned()),
            dealt < 2
        );
        assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20);
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
