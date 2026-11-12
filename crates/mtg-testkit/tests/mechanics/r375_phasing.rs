//! Goal: phasing holds play as printed at tables of two, three and four seats: the creatures Teferi, Timeless Voyager
//! phases out skip their controller's next untap step and phase in at the one after the end of Teferi's controller's
//! next turn; the permanent The Phasing of Zhalfir phases out stays out through its controller's untap step while the
//! Saga's controller controls it; a creature with phasing stays out under Disciple of Caelus Nin and comes back without
//! it; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::phasing::is_phased_out;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn r375_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers until the stack is empty: targets take `target`, a number takes `number`, the rest is passive.
fn r375_settle(
    driver: &mut Driver,
    target: Option<mtg_engine::choose::Target>,
    number: u32,
) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseTargets(_) if target.is_some() => {
                driver.choose_targets(&[target.unwrap()])?;
            }
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(number)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// Seat 0's first main phase of turn 2; every seat has a library.
fn r375_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| bear()));
    }
    scenario
}

/// Whether the seat's Grizzly Bears (any one of them) is phased out.
fn r375_bear_out(driver: &Driver) -> bool {
    driver
        .state()
        .zone(mtg_engine::zones::Zone::PhasedOut)
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == "Grizzly Bears")
}

#[rstest]
fn r375_teferi_holds_the_creatures_through_the_end_of_his_next_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let teferi = with_counters(
        ready(card("Teferi, Timeless Voyager").unwrap()),
        CounterKind::Loyalty,
        8,
    );
    let scenario = r375_table(seats)
        .battlefield(0, [teferi])
        .battlefield(1, [ready(bear()), ready(bear())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Teferi, Timeless Voyager", 2)?;
    let them = driver.target_player(1);
    r375_settle(&mut driver, Some(them), 0)?;
    assert!(r375_bear_out(&driver));
    // Seat 1's next untap step: held by the rule.
    driver.idle_until(3, Step::Upkeep)?;
    assert!(r375_bear_out(&driver));
    // Seat 0's next turn ends the rule; seat 1's untap step after it phases them in.
    let seats = u32::try_from(seats).unwrap();
    driver.idle_until(2 + seats, Step::Upkeep)?;
    assert!(r375_bear_out(&driver));
    driver.idle_until(3 + seats, Step::Upkeep)?;
    assert!(!r375_bear_out(&driver));
    r375_replays(&driver);
    Ok(())
}

#[rstest]
fn r375_zhalfir_holds_while_its_controller_controls_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = r375_table(seats)
        .battlefield(0, (0..4).map(|_| ready(island())))
        .battlefield(1, [ready(bear())])
        .hand(0, [card("The Phasing of Zhalfir").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("The Phasing of Zhalfir")?;
    let bear = driver.target_named("Grizzly Bears")?;
    r375_settle(&mut driver, Some(bear), 1)?;
    assert!(r375_bear_out(&driver));
    driver.idle_until(3, Step::Upkeep)?;
    assert!(r375_bear_out(&driver));
    r375_replays(&driver);
    Ok(())
}

#[rstest]
fn r375_disciple_keeps_phasing_creatures_out(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] disciple: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Breezekeeper").unwrap())];
    if disciple {
        mine.push(ready(card("Disciple of Caelus Nin").unwrap()));
    }
    let mut driver = Driver::from_scenario(r375_table(seats).battlefield(0, mine))?;
    let breeze = driver.id_of("Breezekeeper")?;
    let seats = u32::try_from(seats).unwrap();
    driver.idle_until(2 + seats, Step::Upkeep)?;
    assert!(is_phased_out(driver.state(), breeze));
    driver.idle_until(2 + 2 * seats, Step::Upkeep)?;
    assert_eq!(is_phased_out(driver.state(), breeze), disciple);
    assert_eq!(driver.state().turn().active(), pid(0));
    r375_replays(&driver);
    Ok(())
}
