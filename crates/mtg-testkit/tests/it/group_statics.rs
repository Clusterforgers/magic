//! Goal: the statics that act on a group (permanents that enter tapped, a keyword for the creatures with a counter, ward for the other creatures, a skipped draw step) work at every table size: they reach exactly the objects and the players their words name, and the draw step of a player who skips it never begins.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics};
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

/// Whether the named permanent has a keyword of that name.
fn has(driver: &Driver, name: &str, keyword: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    keywords_of(driver.state(), id)
        .iter()
        .any(|held| held.name() == keyword)
}

/// Every seat has forests to draw from.
fn libraries(seats: usize, scenario: Scenario) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library_named(seat, vec!["Forest"; 12])
    })
}

/// Passes priority from the upkeep to the first main phase and names the steps that began on the way.
fn steps_to_main(driver: &mut Driver) -> Result<Vec<Step>, ScriptError> {
    let mut began = Vec::new();
    for _ in 0..60 {
        if driver.state().turn().step() == Step::Main1 {
            return Ok(began);
        }
        for event in driver.pass()? {
            if let Event::StepBegan { step } = event {
                began.push(step);
            }
        }
    }
    Err(ScriptError::Stuck("the first main phase".into()))
}

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

#[rstest]
fn a_player_who_skips_the_draw_step_never_begins_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] holder: usize,
) -> Result<(), ScriptError> {
    // Seat 0 is the active player; Wild Wasteland is seat `holder`'s, and seat 0 skips only when it is theirs.
    let scenario = libraries(seats, Scenario::new(seats).at(5, 0, Step::Upkeep))
        .battlefield_named(holder, ["Wild Wasteland"]);
    let mut driver = Driver::from_scenario(scenario)?;
    let began = steps_to_main(&mut driver)?;
    if holder == 0 {
        assert!(!began.contains(&Step::Draw), "{began:?}");
        assert_eq!(hand_size(&driver, 0), 0);
    } else {
        assert!(began.contains(&Step::Draw), "{began:?}");
        assert_eq!(hand_size(&driver, 0), 1);
    }
    Ok(())
}

#[rstest]
fn the_draw_step_of_every_other_turn_is_played(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // It is seat 1's turn and seat 0 holds the permanent: seat 1 draws.
    let scenario = libraries(seats, Scenario::new(seats).at(6, 1, Step::Upkeep))
        .battlefield_named(0, ["Wild Wasteland"]);
    let mut driver = Driver::from_scenario(scenario)?;
    let began = steps_to_main(&mut driver)?;
    assert!(began.contains(&Step::Draw), "{began:?}");
    assert_eq!(hand_size(&driver, 1), 1);
    assert_eq!(hand_size(&driver, 0), 0);
    Ok(())
}

/// Seat `caster` casts a Bear on their own turn; the stack is then resolved.
fn bear_enters(seats: usize, caster: usize, holder: usize) -> Result<Driver, ScriptError> {
    let lands = [ready(forest()), ready(forest())];
    let scenario = Scenario::new(seats)
        .at(5, caster, Step::Main1)
        .battlefield_named(holder, ["Authority of the Consuls"])
        .battlefield(caster, lands)
        .hand_named(caster, ["Grizzly Bears"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Grizzly Bears")?;
    for _ in 0..(4 * seats) {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(driver)
}

#[rstest]
fn creatures_of_the_opponents_enter_tapped_and_your_own_do_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] caster: usize,
) -> Result<(), ScriptError> {
    // Seat 0 holds Authority of the Consuls; seat 1 or seat 0 casts the Bear.
    let driver = bear_enters(seats, caster, 0)?;
    let bear = driver.id_of("Grizzly Bears")?;
    let tapped = driver
        .state()
        .object(bear)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(tapped, caster != 0, "caster {caster}");
    Ok(())
}

#[rstest]
fn the_last_seat_is_an_opponent_too(#[values(3, 4)] seats: usize) -> Result<(), ScriptError> {
    let driver = bear_enters(seats, seats - 1, 0)?;
    let bear = driver.id_of("Grizzly Bears")?;
    assert!(
        driver
            .state()
            .object(bear)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    Ok(())
}

#[rstest]
fn each_creature_with_a_counter_has_trample(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let counter =
        |name: &str| with_counters(ready(card(name).unwrap()), CounterKind::PlusOnePlusOne, 1);
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                counter("Duskshell Crawler"),
                counter("Grizzly Bears"),
                ready(card("Jungle Lion").unwrap()),
            ],
        )
        .battlefield(1, [counter("Goblin Chieftain")]);
    let driver = Driver::from_scenario(scenario)?;
    // The bearer of the static ability and another creature with a counter have it; one without and a foe's do not.
    assert!(has(&driver, "Duskshell Crawler", "Trample"));
    assert!(has(&driver, "Grizzly Bears", "Trample"));
    assert!(!has(&driver, "Jungle Lion", "Trample"));
    assert!(!has(&driver, "Goblin Chieftain", "Trample"));
    Ok(())
}

#[rstest]
fn the_bearer_of_the_static_ability_needs_a_counter_too(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Pridemalkin").unwrap()),
            ready(card("Grizzly Bears").unwrap()),
        ],
    );
    let driver = Driver::from_scenario(scenario)?;
    assert!(!has(&driver, "Pridemalkin", "Trample"), "no counter yet");
    assert!(!has(&driver, "Grizzly Bears", "Trample"));
    Ok(())
}

#[rstest]
fn other_creatures_you_control_have_ward(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Star Whale").unwrap()),
                ready(card("Grizzly Bears").unwrap()),
            ],
        )
        .battlefield(1, [ready(card("Jungle Lion").unwrap())]);
    let driver = Driver::from_scenario(scenario)?;
    assert!(has(&driver, "Grizzly Bears", "Ward"));
    assert!(!has(&driver, "Star Whale", "Ward"), "other creatures only");
    assert!(!has(&driver, "Jungle Lion", "Ward"), "a foe's creature");
    let bear = characteristics(driver.state(), driver.id_of("Grizzly Bears")?).unwrap();
    assert!(bear.is_creature());
    Ok(())
}
