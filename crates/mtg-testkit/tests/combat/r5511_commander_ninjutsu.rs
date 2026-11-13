//! Goal: commander ninjutsu (CR 702.49d) works like ninjutsu from the hand and also from the command zone: with Yuriko,
//! the Tiger's Shadow in the command zone an unblocked attacker returns to its owner's hand and Yuriko enters tapped and
//! attacking the same player, without commander tax and without being cast, and deals its combat damage; plain ninjutsu
//! offers nothing from the command zone; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{on_battlefield, pid};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::format::Commander;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 has `commander` in its command zone, Grizzly Bears and an Island and a Swamp; every seat has a library.
fn scenario(seats: usize, commander: &str) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .format(Arc::new(Commander))
            .commanders(0, [mtg_testkit::corpus::card(commander).unwrap()])
            .battlefield(
                0,
                [
                    ready(island()),
                    ready(swamp()),
                    ready(mtg_testkit::corpus::card("Grizzly Bears").unwrap()),
                ],
            ),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    )
}

/// Seat 0 attacks seat `defender` with Grizzly Bears and nobody blocks; the game stops in the declare blockers step.
fn to_blockers(driver: &mut Driver, defender: usize) -> Result<(), ScriptError> {
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", defender)])?;
    loop {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            driver.answer(Answer::DeclareBlockers(Vec::new()))?;
        } else if driver.state().turn().step() == Step::DeclareBlockers
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        } else {
            driver.answer_passively()?;
        }
    }
}

#[rstest]
fn r5511_yuriko_enters_from_the_command_zone_tapped_and_attacking(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let yuriko = "Yuriko, the Tiger's Shadow";
    let defender = seats - 1;
    let mut driver = Driver::from_scenario(scenario(seats, yuriko))?;
    let life = driver.state().player(pid(defender)).unwrap().life();
    to_blockers(&mut driver, defender)?;
    driver.begin_activation(yuriko)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
            unreachable!()
        };
        let pick = options.from.clone();
        driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(pick)))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let id = on_battlefield(driver.state(), yuriko).expect("Yuriko entered");
    assert_eq!(driver.state().zone(Zone::Command(pid(0))), []);
    assert_eq!(
        driver
            .state()
            .combat()
            .attackers()
            .iter()
            .map(|a| (a.object(), a.target()))
            .collect::<Vec<_>>(),
        [(id, AttackTarget::Player(pid(defender)))]
    );
    driver.idle_until(1, Step::End)?;
    settle(&mut driver)?;
    let after = driver.state().player(pid(defender)).unwrap().life();
    assert!(after < life, "Yuriko dealt combat damage");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r5511_plain_ninjutsu_is_not_offered_from_the_command_zone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ninja = "Ninja of the Deep Hours";
    let mut driver = Driver::from_scenario(scenario(seats, ninja))?;
    to_blockers(&mut driver, seats - 1)?;
    assert_eq!(driver.offered_abilities(ninja)?, [] as [usize; 0]);
    Ok(())
}
