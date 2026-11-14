//! Goal: at two to four seats, River Song's controller draws from the bottom of their library while every other player
//! draws from the top, and each scry or surveil of an opponent puts a +1/+1 counter on River Song and deals damage equal
//! to its power to that player, never for its controller's own scry.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact with "{0}: `effect`".
fn r5508_tool(name: &str, effect: &str) -> Arc<CardDefinition> {
    let json = format!(r#"{{"Activated":{{"cost":"{{0}}","effects":[{effect}]}}}}"#);
    let ability: Ability = serde_json::from_str(&json).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

/// Answers passively until the stack and the waiting triggers are empty.
fn r5508_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 controls River Song; seat `active` controls the tool and it is their first main phase; every library is
/// "Top" over three "Middle" cards over "Bottom".
fn r5508_table(
    seats: usize,
    active: usize,
    tool: Arc<CardDefinition>,
) -> Result<Driver, ScriptError> {
    let library = || {
        let mut cards = vec![free_creature("Top", 1, 1)];
        cards.extend((0..3).map(|_| free_creature("Middle", 1, 1)));
        cards.push(free_creature("Bottom", 1, 1));
        cards
    };
    let mut scenario = Scenario::new(seats).at(2, active, Step::Main1);
    for seat in 0..seats {
        scenario = scenario.library(seat, library());
    }
    let mut mine = vec![ready(card("River Song").unwrap())];
    if active == 0 {
        mine.push(ready(tool));
    } else {
        scenario = scenario.battlefield(active, [ready(tool)]);
    }
    Driver::from_scenario(scenario.battlefield(0, mine))
}

/// Activates the tool of the active seat once and lets everything resolve.
fn r5508_use(driver: &mut Driver, tool: &str) -> Result<(), ScriptError> {
    driver.begin_activation(tool)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r5508_settle(driver)
}

fn r5508_hand_names(driver: &Driver, seat: usize) -> Vec<String> {
    let state = driver.state();
    state
        .zone(Zone::Hand(PlayerId::from_index(seat)))
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|object| object.card().name.clone())
        .collect()
}

fn r5508_song(driver: &Driver) -> u32 {
    let id = driver.id_of("River Song").unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| {
            object
                .permanent()
                .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne))
        })
        .unwrap()
}

#[rstest]
#[case::mine_two_seats(2, 0, "Bottom")]
#[case::mine_four_seats(4, 0, "Bottom")]
#[case::theirs_three_seats(3, 1, "Top")]
#[case::theirs_four_seats(4, 3, "Top")]
fn r5508_river_songs_controller_draws_from_the_bottom(
    #[case] seats: usize,
    #[case] drawer: usize,
    #[case] drawn: &str,
) -> Result<(), ScriptError> {
    let mut driver = r5508_table(seats, drawer, r5508_tool("Book", r#"{"Draw":{}}"#))?;
    r5508_use(&mut driver, "Book")?;
    assert_eq!(r5508_hand_names(&driver, drawer), vec![drawn.to_owned()]);
    Ok(())
}

#[rstest]
#[case::scry_two_seats(2, 1, r#"{"Scry":{"count":1}}"#, 1)]
#[case::surveil_three_seats(3, 2, r#"{"Surveil":{"count":1}}"#, 1)]
#[case::scry_four_seats(4, 3, r#"{"Scry":{"count":2}}"#, 1)]
#[case::own_scry_two_seats(2, 0, r#"{"Scry":{"count":1}}"#, 0)]
fn r5508_an_opponents_scry_or_surveil_grows_river_song(
    #[case] seats: usize,
    #[case] seer: usize,
    #[case] effect: &str,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let mut driver = r5508_table(seats, seer, r5508_tool("Lens", effect))?;
    let life = driver
        .state()
        .player(PlayerId::from_index(seer))
        .unwrap()
        .life();
    r5508_use(&mut driver, "Lens")?;
    assert_eq!(r5508_song(&driver), counters);
    let lost = life
        - driver
            .state()
            .player(PlayerId::from_index(seer))
            .unwrap()
            .life();
    // River Song is a 2/2: with its counter it deals 3.
    assert_eq!(lost, if counters == 1 { 3 } else { 0 });
    Ok(())
}
