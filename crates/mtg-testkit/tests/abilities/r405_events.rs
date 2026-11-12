//! Goal: the r40-5 trigger events play as printed at tables of two, three and four seats: Vizier of the Anointed draws
//! when its controller activates an embalm ability and not when an opponent does; Case File Auditor triggers when its
//! controller solves a Case; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::Event;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers passively until the stack and the pending triggers are empty, gathering the events.
fn r405_settle(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(events);
        }
        events.extend(driver.answer(Driver::passive_answer(&decision))?);
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r405_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn r405_hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// Seat `embalmer` activates the embalm ability of Aven Initiate in its graveyard with seven Islands; seat 0 controls
/// Vizier of the Anointed and draws only when it is the one who activates.
#[rstest]
#[case::mine_at_two(2, 0, 1)]
#[case::mine_at_four(4, 0, 1)]
#[case::an_opponents(3, 2, 0)]
fn the_vizier_draws_for_its_controllers_embalm(
    #[case] seats: usize,
    #[case] embalmer: usize,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, embalmer, Step::Main1)
        .battlefield(0, [ready(card("Vizier of the Anointed").unwrap())])
        .battlefield(embalmer, (0..7).map(|_| ready(island())))
        .graveyard(embalmer, [card("Aven Initiate").unwrap()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let before = r405_hand(&driver, 0);
    let ability = *driver
        .offered_abilities("Aven Initiate")?
        .first()
        .expect("embalm is offered");
    driver.begin_ability("Aven Initiate", ability)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r405_settle(&mut driver)?;
    assert_eq!(r405_hand(&driver, 0), before + drawn);
    r405_replays(&driver);
    Ok(())
}

/// A Case solved at the end step of its controller sets off Case File Auditor's look at the library.
#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn the_auditor_triggers_when_a_case_is_solved(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut mine = vec![
        ready(card("Case File Auditor").unwrap()),
        ready(card("Case of the Locked Hothouse").unwrap()),
    ];
    mine.extend((0..7).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main2)
        .battlefield(0, mine);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| bear()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let auditor = driver.id_of("Case File Auditor")?;
    let mut events = Vec::new();
    for _ in 0..40 {
        if driver.state().turn().step() == Step::End
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
            && events
                .iter()
                .any(|e| matches!(e, Event::Solved { .. }))
        {
            break;
        }
        let decision = driver.decision()?.clone();
        events.extend(driver.answer(Driver::passive_answer(&decision))?);
    }
    assert!(events.iter().any(|e| matches!(e, Event::Solved { .. })));
    let triggered = events
        .iter()
        .filter(|e| matches!(e, Event::AbilityTriggered(t) if t.source.id == auditor))
        .count();
    assert_eq!(triggered, 1);
    r405_replays(&driver);
    Ok(())
}
