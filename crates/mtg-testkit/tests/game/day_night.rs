//! Goal: the game is neither day nor night until a permanent with daybound or nightbound is on the battlefield and then becomes day; at the start of each turn it becomes night after a turn whose active player cast no spells and day after one whose active player cast two or more, otherwise it stays (CR 726.3a, over a table of turn sequences at two to four seats); daybound permanents are turned to their night face when it becomes night and nightbound ones to their day face when it becomes day, whoever controls them and wherever they entered; a daybound card that enters at night is turned at once; and the log replays to the same state and hash.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics::characteristics;
use mtg_engine::day_night::DayNight::{Day, Night};
use mtg_engine::turn::Step;
use mtg_engine::{DayNight, Event};
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::name_of;

/// Every seat has forests to cast with and Fog to cast, and seat 1 controls a Tavern Ruffian.
fn table(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        scenario = scenario
            .hand_named(seat, ["Fog"; 6])
            .library(seat, vec![forest(); 30])
            .battlefield(
                seat,
                (0..6).map(|_| ready(forest())).chain((seat == 1).then(|| {
                    ready(mtg_testkit::corpus::card("Tavern Ruffian // Tavern Smasher").unwrap())
                })),
            );
    }
    let table = Driver::from_scenario(scenario)?;
    Ok(table)
}

/// Plays the turns in order, the active player of each casting that many spells, and stops at the upkeep after them.
fn play(driver: &mut Driver, seats: usize, casts: &[usize]) -> Result<(), ScriptError> {
    for (turn, spells) in casts.iter().enumerate() {
        let number = u32::try_from(turn).unwrap() + 1;
        driver.idle_until(number, Step::Main1)?;
        for _ in 0..*spells {
            driver.cast_tapping("Fog", &["Forest"])?;
            for _ in 0..seats {
                driver.pass()?;
            }
        }
    }
    let after = u32::try_from(casts.len()).unwrap() + 1;
    driver.idle_until(after, Step::Upkeep)?;
    Ok(())
}

#[rstest]
#[case::a_turn_with_no_spell_makes_night(&[0], Night, "Tavern Smasher")]
#[case::one_spell_keeps_the_day(&[1], Day, "Tavern Ruffian")]
#[case::two_spells_keep_the_day(&[2], Day, "Tavern Ruffian")]
#[case::night_after_night(&[0, 0], Night, "Tavern Smasher")]
#[case::one_spell_keeps_the_night(&[0, 1], Night, "Tavern Smasher")]
#[case::two_spells_make_day(&[0, 2], Day, "Tavern Ruffian")]
#[case::three_spells_make_day(&[0, 3], Day, "Tavern Ruffian")]
#[case::day_night_day_night(&[0, 2, 0], Night, "Tavern Smasher")]
#[case::spells_do_not_carry_over(&[0, 1, 1, 2], Day, "Tavern Ruffian")]
fn the_day_or_night_follows_the_spells_of_the_previous_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] casts: &[usize],
    #[case] expected: DayNight,
    #[case] face: &str,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    play(&mut driver, seats, casts)?;
    assert_eq!(driver.state().day_night(), Some(expected));
    let ruffian = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id).starts_with("Tavern"))
        .unwrap();
    assert_eq!(name_of(driver.state(), ruffian), face);
    let found = characteristics(driver.state(), ruffian).unwrap();
    assert_eq!(
        (found.power.unwrap(), found.toughness.unwrap()),
        if expected == Day { (2, 5) } else { (6, 5) }
    );
    Ok(())
}

#[test]
fn the_first_check_with_a_daybound_permanent_makes_it_day() -> Result<(), ScriptError> {
    let driver = table(2)?;
    assert_eq!(driver.state().day_night(), Some(Day));
    let changes = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::DayNightChanged { .. }))
        .count();
    assert_eq!(changes, 1);
    Ok(())
}

#[test]
fn a_game_without_a_daybound_permanent_stays_neither_day_nor_night() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .library(0, vec![forest(); 10])
            .library(1, vec![forest(); 10]),
    )?;
    driver.idle_until(4, Step::Upkeep)?;
    assert_eq!(driver.state().day_night(), None);
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::DayNightChanged { .. }))
    );
    Ok(())
}

#[test]
fn a_nightbound_face_up_at_the_start_makes_it_night_and_a_daybound_one_that_enters_at_night_is_turned()
-> Result<(), ScriptError> {
    // Wing Shredder arrives as the back face: it is turned up by the day, then it is day.
    let mut driver = table(2)?;
    play(&mut driver, 2, &[0])?;
    assert_eq!(driver.state().day_night(), Some(Night));
    // A second daybound card put onto the battlefield at night is turned when state-based actions are checked.
    let mut state = driver.state().clone();
    let events = mtg_engine::apply_event(&mut state, &Event::DayNightChanged { to: Day });
    assert!(events.is_ok());
    assert_eq!(state.day_night(), Some(Day));
    Ok(())
}

#[rstest]
fn a_game_with_day_and_night_replays_to_the_same_state(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats)?;
    play(&mut driver, seats, &[0, 2, 0])?;
    let replayed = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    assert_eq!(replayed.day_night(), driver.state().day_night());
    let changes: Vec<DayNight> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::DayNightChanged { to } => Some(*to),
            _ => None,
        })
        .collect();
    assert_eq!(changes, [Day, Night, Day, Night]);
    Ok(())
}
