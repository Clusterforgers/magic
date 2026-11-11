//! Goal: Attractions (CR 717) play at tables of two to four seats: the deck lies face down in its owner's command zone and is shuffled before the game, opening puts its top cards onto the battlefield, the active player rolls to visit as the precombat main phase begins only while they control an Attraction, each Attraction with the result lit up has its visit ability triggered, a destroyed Attraction goes to the junkyard in the command zone face up, the other seats are untouched, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::attraction::Lights;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{mountain, un33_attraction, un33_opener};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 brings three Attractions lit at `lit`, holds an opener of two and rolls nothing yet; it has Mountains for
/// Smash to Smithereens.
fn table(seats: usize, lit: &[u32]) -> Scenario {
    let deck = (0..3).map(|n| {
        (
            un33_attraction(&format!("Ride {n}")),
            Lights::of(lit.iter().copied()),
        )
    });
    Scenario::new(seats)
        .at(1, 0, Step::Upkeep)
        .attractions(0, deck)
        .hand(
            0,
            [
                un33_opener("Open Two", Some(2)),
                mtg_testkit::corpus::card("Smash to Smithereens").unwrap(),
            ],
        )
        .battlefield(0, [ready(mountain()), ready(mountain())])
}

/// Answers passively until the stack and the pending triggers are empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        driver.answer_passively()?;
    }
    Ok(())
}

fn rolls(driver: &Driver) -> Vec<(u32, usize)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::RolledToVisit {
                result, visited, ..
            } => Some((*result, visited.len())),
            _ => None,
        })
        .collect()
}

fn on_battlefield(driver: &Driver) -> Vec<mtg_core::ids::ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().lights(*id).is_some())
        .collect()
}

#[rstest]
fn opened_attractions_are_visited_as_the_precombat_main_phase_begins(
    #[values(2, 3, 4)] seats: usize,
    #[values(&[1, 2, 3, 4, 5, 6][..], &[2, 6][..])] lit: &[u32],
) -> Result<(), ScriptError> {
    let scenario = table(seats, lit);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(driver.state().attraction_deck(pid(0)).len(), 3);
    driver.cast("Open Two")?;
    settle(&mut driver)?;
    assert_eq!(on_battlefield(&driver).len(), 2);
    assert_eq!(driver.state().attraction_deck(pid(0)).len(), 1);
    assert!(rolls(&driver).is_empty(), "no roll before the main phase");
    driver.pass_until(Step::Main1)?;
    // The visit abilities wait on the stack; the hand holds the card of the draw step.
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    settle(&mut driver)?;
    let [(result, visited)] = rolls(&driver)[..] else {
        panic!("one roll: {:?}", rolls(&driver));
    };
    let expected = if lit.contains(&result) { 2 } else { 0 };
    assert_eq!(visited, expected, "rolled {result}");
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + expected,
        "each visit draws a card"
    );
    for seat in 1..seats {
        assert_eq!(driver.state().attraction_deck(pid(seat)).len(), 0);
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[test]
fn no_attraction_no_roll() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, &[2]))?;
    driver.pass_until(Step::Main1)?;
    assert_eq!(rolls(&driver), []);
    Ok(())
}

#[test]
fn a_destroyed_attraction_goes_to_the_junkyard() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, &[2]))?;
    driver.cast("Open Two")?;
    settle(&mut driver)?;
    let [first, second] = on_battlefield(&driver)[..] else {
        panic!("two opened");
    };
    driver.pass_until(Step::Main1)?;
    settle(&mut driver)?;
    driver.begin_cast("Smash to Smithereens")?;
    let name = driver.state().object(first).unwrap().card().name.clone();
    let target = driver.target_named(&name)?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let gone: Vec<_> = [first, second]
        .into_iter()
        .filter(|id| driver.state().object(*id).unwrap().zone() != Zone::Battlefield)
        .collect();
    let [gone] = gone[..] else {
        panic!("one destroyed");
    };
    let card = driver.state().object(gone).unwrap();
    assert_eq!(card.zone(), Zone::Command(pid(0)));
    assert_eq!(card.face_down(), None);
    assert!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .all(|id| *id != gone)
    );
    assert!(!driver.state().in_attraction_deck(gone));
    Ok(())
}
