//! Goal: Hundred-Battle Veteran cast from its owner's graveyard enters with a finality counter and one cast from the hand
//! enters without; Yuna, Grand Summoner moves the counters of another permanent its controller controlled that went
//! to the graveyard, as many +1/+1 counters as it had, onto a target creature, and nothing for one without counters;
//! at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::on_battlefield;

/// Answers passively until the stack and the waiting triggers are empty, choosing `target` for any target.
fn r5523_settle(driver: &mut Driver, target: Option<&str>) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        match (&decision.kind, target) {
            (DecisionKind::ChooseTargets(_), Some(name)) => {
                let chosen = driver.target_named(name)?;
                driver.choose_targets(&[chosen])?;
            }
            (DecisionKind::YesNo(_), Some(_)) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn r5523_veteran_from_the_graveyard_has_finality(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] from_graveyard: bool,
) -> Result<(), ScriptError> {
    let veteran = card("Hundred-Battle Veteran").unwrap();
    let table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..4).map(|_| ready(swamp())));
    let table = if from_graveyard {
        table.graveyard(0, [veteran])
    } else {
        table.hand(0, [veteran])
    };
    let mut driver = Driver::from_scenario(table)?;
    driver.begin_cast("Hundred-Battle Veteran")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r5523_settle(&mut driver, None)?;
    let id = on_battlefield(driver.state(), "Hundred-Battle Veteran").expect("it resolved");
    let permanent = driver.state().object(id).unwrap().permanent().unwrap();
    assert_eq!(
        permanent.counters_of(&CounterKind::Other("finality".into())),
        u32::from(from_graveyard)
    );
    Ok(())
}

#[rstest]
#[case::two_counters(2, 2)]
#[case::one_counter(1, 1)]
#[case::near_miss_no_counters(0, 0)]
fn r5523_yuna_moves_the_counters_of_what_died(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: u32,
    #[case] moved: u32,
) -> Result<(), ScriptError> {
    let fallen = if held > 0 {
        with_counters(
            ready(free_creature("Fallen", 0, 3)),
            CounterKind::PlusOnePlusOne,
            held,
        )
    } else {
        ready(free_creature("Fallen", 0, 3))
    };
    let table = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Yuna, Grand Summoner").unwrap()),
            ready(free_creature("Heir", 1, 1)),
            fallen,
        ],
    );
    let mut driver = Driver::from_scenario(table)?;
    let fallen = driver.id_of("Fallen")?;
    driver.manual(mtg_engine::manual::ManualAction::Sacrifice { object: fallen })?;
    r5523_settle(&mut driver, Some("Heir"))?;
    let heir = driver.id_of("Heir")?;
    let permanent = driver.state().object(heir).unwrap().permanent().unwrap();
    assert_eq!(permanent.counters_of(&CounterKind::PlusOnePlusOne), moved);
    Ok(())
}
