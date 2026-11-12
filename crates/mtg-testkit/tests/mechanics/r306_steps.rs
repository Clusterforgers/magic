//! Goal: a step added after the current one begins next and only once, its kind's "at the beginning of" triggers see it,
//! and "the first upkeep (end step) of the turn" is false in the added one, so an added step never adds another: a
//! cursed player's turn has two upkeeps and the others one, a creature untapped in its controller's untap step adds one
//! upkeep and an untapped one or one untapped on another turn none, and a blink at the end step adds one end step.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, enchanting, ready, tapped};
use rstest::rstest;

const DOCTOR: &str = r#"{"Triggered":{"when":{"Untapped":{"what":["This"],"untap_step":true}},"effects":[{"AddStep":{"step":"Upkeep"}}]}}"#;
const WATCH: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;

fn card(name: &str) -> Arc<CardDefinition> {
    corpus::card(name).unwrap()
}

/// A creature with "Whenever this becomes untapped during your untap step, you get an additional upkeep step after this
/// step." (The Ninth Doctor).
fn doctor() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(DOCTOR).unwrap();
    Arc::new((*free_creature("Doctor", 1, 1)).clone().with_rules(rules))
}

fn watcher() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(WATCH).unwrap();
    Arc::new((*free_creature("Watcher", 1, 1)).clone().with_rules(rules))
}

fn deck(seat: usize) -> Vec<Arc<CardDefinition>> {
    (0..4)
        .map(|at| free_creature(&format!("Deck {seat} {at}"), 1, 1))
        .collect()
}

/// The steps of kind `step` begun in turn `turn`; the scenarios start in turn 2, which has no `TurnBegan`.
fn begun(driver: &Driver, turn: u32, step: Step) -> usize {
    let mut current = 2;
    let mut count = 0;
    for event in driver.engine().events() {
        match event {
            Event::TurnBegan { number, .. } => current = *number,
            Event::StepBegan { step: began } if current == turn && *began == step => count += 1,
            _ => {}
        }
    }
    count
}

/// Passes until turn `turn` begins its draw step and checks the log replays to the same state.
fn to_draw(driver: &mut Driver, turn: u32) -> Result<(), ScriptError> {
    driver.idle_until(turn, Step::Draw)?;
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

#[rstest]
#[case::the_active_player_at_two_seats(2, 0, 2)]
#[case::another_player_at_two_seats(2, 1, 1)]
#[case::the_active_player_at_four_seats(4, 0, 2)]
#[case::another_player_at_four_seats(4, 2, 1)]
fn paradox_haze_gives_its_player_one_more_upkeep(
    #[case] seats: usize,
    #[case] cursed: usize,
    #[case] upkeeps: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Untap).battlefield(
        0,
        [
            ready(watcher()),
            enchanting(ready(card("Paradox Haze")), cursed),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, deck(seat));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    to_draw(&mut driver, 2)?;
    assert_eq!(begun(&driver, 2, Step::Upkeep), upkeeps);
    assert_eq!(
        counters(&driver, "Watcher") as usize,
        upkeeps,
        "each upkeep triggers"
    );
    Ok(())
}

#[rstest]
#[case::untapped_in_its_untap_step(2, true, 0, 2)]
#[case::already_untapped(2, false, 0, 1)]
#[case::untapped_on_another_turn(2, true, 1, 1)]
#[case::four_seats(4, true, 0, 2)]
fn an_untap_in_your_untap_step_adds_an_upkeep(
    #[case] seats: usize,
    #[case] starts_tapped: bool,
    #[case] active: usize,
    #[case] upkeeps: usize,
) -> Result<(), ScriptError> {
    let doctor = doctor();
    let permanent = if starts_tapped {
        tapped(doctor)
    } else {
        ready(doctor)
    };
    let mut scenario = Scenario::new(seats)
        .at(2, active, Step::Untap)
        .battlefield(0, [permanent]);
    for seat in 0..seats {
        scenario = scenario.library(seat, deck(seat));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    to_draw(&mut driver, 2)?;
    assert_eq!(begun(&driver, 2, Step::Upkeep), upkeeps);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn yshtola_blinks_twice_over_two_end_steps(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main2)
        .battlefield(0, [ready(card("Y'shtola Rhul"))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, deck(seat));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    to_draw(&mut driver, 3)?;
    assert_eq!(begun(&driver, 2, Step::End), 2);
    assert_eq!(begun(&driver, 3, Step::End), 0);
    let blinks = driver
        .engine()
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    to: mtg_engine::zones::Zone::Exile,
                    ..
                }
            )
        })
        .count();
    assert_eq!(blinks, 2, "one blink in each end step");
    Ok(())
}
