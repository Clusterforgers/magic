//! Goal: mill triggers and the radiation rule at tables of two to four seats: the rad mill of seat 0 makes Glowing One gain 1 life for each nonland card, Mirelurk Queen draw and grow once for the batch, Infesting Radroach return from its owner's graveyard, and Strong, the Brutish Thespian turn the loss into a gain for its controller alone; lands milled trigger nothing, and each game replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{life, replays};
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Seat 0 holds three rad counters over a library of a Grizzly Bears to draw, then `top` (a Forest for 'L', Grizzly
/// Bears otherwise, the first on top) and ten more Grizzly Bears; every other seat draws from ten Grizzly Bears. The
/// game starts in seat 0's upkeep of turn 2.
fn w8b_scenario(seats: usize, top: &str) -> Scenario {
    let library: Vec<&str> = std::iter::once("Grizzly Bears")
        .chain(
            top.chars()
                .map(|c| if c == 'L' { "Forest" } else { "Grizzly Bears" }),
        )
        .chain(std::iter::repeat_n("Grizzly Bears", 10))
        .collect();
    (1..seats).fold(
        Scenario::new(seats)
            .at(2, 0, Step::Upkeep)
            .library_named(0, library)
            .counters(0, CounterKind::Other("rad".to_owned()), 3),
        |scenario, seat| scenario.library_named(seat, std::iter::repeat_n("Grizzly Bears", 10)),
    )
}

/// Plays to seat 0's combat, answering "yes" to each may and passively to the rest.
fn w8b_play(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.idle_until(2, Step::Main1)?;
    for _ in 0..200 {
        if driver.state().turn().step() == Step::BeginCombat {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("combat".to_owned()))
}

fn w8b_count(driver: &Driver, zone: Zone) -> usize {
    driver.state().zone(zone).len()
}

/// The zone of the card called `name`.
fn w8b_zone_of(driver: &Driver, name: &str) -> Zone {
    driver
        .state()
        .objects()
        .find(|o| o.card().name == name)
        .unwrap()
        .zone()
}

#[rstest]
#[case::all_nonland("CCC", 3)]
#[case::some_lands("LCL", 1)]
#[case::only_lands("LLL", 0)]
fn w8b_glowing_one_gains_for_each_nonland_card(
    #[values(2, 3, 4)] seats: usize,
    #[case] top: &str,
    #[case] nonland: i32,
) -> Result<(), ScriptError> {
    let scenario = || w8b_scenario(seats, top).battlefield_named(seats - 1, ["Glowing One"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    w8b_play(&mut driver)?;
    assert_eq!(life(&driver, seats - 1), 20 + nonland);
    assert_eq!(
        life(&driver, 0),
        20 - nonland,
        "the rad rule still costs seat 0"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::all_nonland("CCC", true)]
#[case::some_lands("LCL", true)]
#[case::only_lands("LLL", false)]
fn w8b_mirelurk_queen_triggers_once_for_the_batch(
    #[values(2, 3, 4)] seats: usize,
    #[case] top: &str,
    #[case] fires: bool,
) -> Result<(), ScriptError> {
    let queen_seat = seats - 1;
    let scenario = || w8b_scenario(seats, top).battlefield_named(queen_seat, ["Mirelurk Queen"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let hand = w8b_count(&driver, Zone::Hand(PlayerId::from_index(queen_seat)));
    w8b_play(&mut driver)?;
    let drawn = w8b_count(&driver, Zone::Hand(PlayerId::from_index(queen_seat))) - hand;
    let queen = driver.id_of("Mirelurk Queen")?;
    let counters = driver
        .state()
        .object(queen)
        .and_then(|o| o.permanent())
        .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!((drawn, counters), if fires { (1, 1) } else { (0, 0) });
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::a_nonland_card("CLL", true)]
#[case::only_lands("LLL", false)]
fn w8b_infesting_radroach_returns_when_an_opponent_mills(
    #[values(2, 3, 4)] seats: usize,
    #[case] top: &str,
    #[case] returns: bool,
) -> Result<(), ScriptError> {
    let owner = seats - 1;
    let scenario = || w8b_scenario(seats, top).graveyard_named(owner, ["Infesting Radroach"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    w8b_play(&mut driver)?;
    let zone = w8b_zone_of(&driver, "Infesting Radroach");
    let owner = PlayerId::from_index(owner);
    assert_eq!(
        zone,
        if returns {
            Zone::Hand(owner)
        } else {
            Zone::Graveyard(owner)
        }
    );
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn w8b_infesting_radroach_ignores_its_owners_own_mill() -> Result<(), ScriptError> {
    let scenario = || w8b_scenario(2, "CCC").graveyard_named(0, ["Infesting Radroach"]);
    let mut driver = Driver::from_scenario(scenario())?;
    w8b_play(&mut driver)?;
    let zone = w8b_zone_of(&driver, "Infesting Radroach");
    assert_eq!(zone, Zone::Graveyard(PlayerId::from_index(0)));
    Ok(())
}

#[rstest]
#[case::mine(0, 3)]
#[case::an_opponents(1, -3)]
fn w8b_strong_turns_radiation_into_life_for_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[case] holder: usize,
    #[case] change: i32,
) -> Result<(), ScriptError> {
    let scenario =
        || w8b_scenario(seats, "CCC").battlefield_named(holder, ["Strong, the Brutish Thespian"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    w8b_play(&mut driver)?;
    assert_eq!(life(&driver, 0), 20 + change);
    replays(&driver, &setup);
    Ok(())
}
