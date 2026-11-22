//! Goal: a delayed triggered ability whose own event is about no object ("at the beginning of the next end step, return that card") reads `Triggering` as the ability that set it up did (CR 603.7c), at tables of two to four seats: each creature that died returns at the next end step as the card it became (CR 400.7), also an undying creature that died with a +1/+1 counter, whose undying does not trigger; a card that left the graveyard before the end step stays where it went; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls the returner, the given creatures each with a +1/+1 counter and two Mountains, with two Bolts in hand.
fn game(seats: usize, returner: &str, creatures: &[&str]) -> Scenario {
    let mut battlefield = vec![
        ready(card(returner).unwrap()),
        ready(mountain()),
        ready(mountain()),
    ];
    battlefield.extend(
        creatures
            .iter()
            .map(|name| with_counters(ready(card(name).unwrap()), CounterKind::PlusOnePlusOne, 1)),
    );
    Scenario::new(seats)
        .hand(
            0,
            [
                card("Lightning Bolt").unwrap(),
                card("Lightning Bolt").unwrap(),
            ],
        )
        .battlefield(0, battlefield)
}

/// Answers every question passively until nothing is on the stack or resolving and priority is asked.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle && matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("settling".into()))
}

/// Seat 0 bolts each creature named, one after the other.
fn bolt(driver: &mut Driver, creatures: &[&str]) -> Result<(), ScriptError> {
    for name in creatures {
        let at = driver.target_named(name)?;
        driver.cast_at("Lightning Bolt", &[at])?;
        settle(driver)?;
    }
    Ok(())
}

/// The +1/+1 counters on each permanent named `name` seat 0 controls.
fn returned(driver: &Driver, name: &str) -> Vec<u32> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name && object.controller() == pid(0))
        .map(|object| {
            object.permanent().map_or(0, |permanent| {
                permanent.counters_of(&CounterKind::PlusOnePlusOne)
            })
        })
        .collect()
}

fn replays(driver: &Driver, scenario: Scenario) {
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn each_creature_that_died_returns_at_the_next_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[values("Marchesa, the Black Rose", "Avacyn, Angel of Horror")] returner: &str,
    #[values(&["Young Wolf"][..], &["Young Wolf", "Strangleroot Geist"][..])] dying: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(seats, returner, dying))?;
    bolt(&mut driver, dying)?;
    let mut graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    graveyard.retain(|name| name != "Lightning Bolt");
    graveyard.sort();
    let mut died: Vec<String> = dying.iter().map(|name| (*name).to_owned()).collect();
    died.sort();
    assert_eq!(
        graveyard, died,
        "undying does not return a creature that had a counter"
    );
    assert_eq!(
        driver.state().delayed().len(),
        dying.len(),
        "one return waits for each"
    );

    let turn = driver.state().turn().number();
    driver.idle_until(turn, Step::End)?;
    settle(&mut driver)?;
    for name in dying {
        assert_eq!(
            returned(&driver, name),
            [0],
            "{name} returned once under seat 0, as a new object"
        );
    }
    let mut graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    graveyard.retain(|name| name != "Lightning Bolt");
    assert!(graveyard.is_empty(), "{graveyard:?}");
    replays(&driver, game(seats, returner, dying));
    Ok(())
}

#[rstest]
fn a_card_that_left_the_graveyard_before_the_end_step_stays_where_it_went(
    #[values(2, 3, 4)] seats: usize,
    #[values("Marchesa, the Black Rose", "Avacyn, Angel of Horror")] returner: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(seats, returner, &["Young Wolf"]))?;
    bolt(&mut driver, &["Young Wolf"])?;
    let wolf = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == "Young Wolf")
        .unwrap();
    driver.manual_by(
        pid(0),
        &ManualAction::Move {
            object: wolf,
            to: ManualZone::Exile,
        },
    )?;
    let turn = driver.state().turn().number();
    driver.idle_until(turn, Step::End)?;
    settle(&mut driver)?;
    assert_eq!(returned(&driver, "Young Wolf"), [] as [u32; 0]);
    assert_eq!(names(driver.state(), Zone::Exile), ["Young Wolf"]);
    replays(&driver, game(seats, returner, &["Young Wolf"]));
    Ok(())
}
