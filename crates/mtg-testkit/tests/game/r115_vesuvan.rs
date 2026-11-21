//! Goal: Vesuvan Shapeshifter at 2 to 4 seats (CR 702.37e, 707.2, 708.8): turned face up for its morph cost, its
//! controller may choose another creature, never itself, and it becomes a copy of that creature with "At the beginning
//! of your upkeep, you may turn this creature face down"; choosing none leaves it the Shapeshifter. Turned face down,
//! the copy ends, so turned up again it may copy another creature; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::events::Event;
use mtg_engine::zones::Zone;
use mtg_engine::{apply_event, replay};
use mtg_testkit::cards::{forest, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};

/// Seat 0 holds Vesuvan Shapeshifter with three Forests, two Islands and a 2/2 Bear; each other seat has a 7/7 Giant.
fn table(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![card("Vesuvan Shapeshifter").unwrap()])
        .battlefield(
            0,
            vec![
                ready(forest()),
                ready(forest()),
                ready(forest()),
                ready(island()),
                ready(island()),
                ready(free_creature("Bear", 2, 2)),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
        if seat > 0 {
            scenario = scenario.battlefield(
                seat,
                vec![ready(free_creature(&format!("Giant {seat}"), 7, 7))],
            );
        }
    }
    Driver::from_scenario(scenario)
}

/// Seat 0 casts the Shapeshifter face down with the Forests and lets it resolve; the face-down permanent.
fn face_down(driver: &mut Driver) -> Result<ObjectId, ScriptError> {
    driver.begin_cast("Vesuvan Shapeshifter")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, "Cast face down")?;
    }
    driver.pay_tapping(&["Forest", "Forest", "Forest"], 0)?;
    for _ in 0..10 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .expect("a face-down permanent"))
}

/// Turns `id` face up, copying the creature named `copy` (none when `None`); the options the copy choice offered.
fn turn_up(
    driver: &mut Driver,
    id: ObjectId,
    copy: Option<&str>,
) -> Result<Vec<ObjectId>, ScriptError> {
    driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
    let mut offered = Vec::new();
    loop {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                offered.clone_from(&options.from);
                let take = match copy {
                    Some(name) => vec![driver.id_of(name)?],
                    None => Vec::new(),
                };
                driver.answer(Answer::ChooseCards(CardsChoice(take)))?;
            }
            DecisionKind::PayMana(_) => {
                pay(driver)?;
            }
            _ => break,
        }
    }
    Ok(offered)
}

fn name_of(driver: &Driver, id: ObjectId) -> String {
    crate::common::name_of(driver.state(), id)
}

#[rstest]
fn r115_turned_up_it_copies_the_chosen_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some("Bear"), Some("Giant 1"))] copy: Option<&str>,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    let id = face_down(&mut driver)?;
    let offered = turn_up(&mut driver, id, copy)?;
    assert!(!offered.contains(&id), "it never copies itself");
    assert_eq!(offered.len(), seats, "the Bear and every Giant");
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.face_down(), None);
    assert_eq!(name_of(&driver, id), copy.unwrap_or("Vesuvan Shapeshifter"));
    let chars = characteristics(driver.state(), id).unwrap();
    let power = match copy {
        Some("Bear") => 2,
        Some(_) => 7,
        None => 0,
    };
    assert_eq!(chars.power, Some(power));
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn r115_turned_face_down_the_copy_ends(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    let id = face_down(&mut driver)?;
    turn_up(&mut driver, id, Some("Giant 1"))?;
    assert_eq!(name_of(&driver, id), "Giant 1");
    let mut turned = driver.state().clone();
    let down = Event::TurnedFaceDown {
        object: id,
        player: crate::common::pid(0),
        horror: false,
    };
    apply_event(&mut turned, &down).unwrap();
    assert_eq!(
        characteristics(&turned, id).unwrap().power,
        Some(2),
        "a face-down 2/2"
    );
    let up = Event::TurnedFaceUp {
        object: id,
        player: crate::common::pid(0),
        x: 0,
    };
    apply_event(&mut turned, &up).unwrap();
    assert_eq!(
        crate::common::name_of(&turned, id),
        "Vesuvan Shapeshifter",
        "the copy ended as it turned face down"
    );
    Ok(())
}
