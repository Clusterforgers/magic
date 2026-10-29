//! Goal: the initiative (CR 725) works at tables of two, three and four seats: taking it ventures into Undercity through the stack (a second take ventures again, and a venture cast in response resolves first), its holder ventures at the beginning of their upkeep, combat damage to the holder passes it to the attacker's controller with a venture, a holder who leaves the game passes it to the active player, Undercity is walked to its completion, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, island, w11d_venturer, w15a_initiator};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds three Initiatives and a Venture, seat 1 an Initiative and a ready Raider; every seat has an Island.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        let mut permanents = vec![ready(island())];
        if seat == 1 {
            permanents.push(ready(free_creature("Raider", 2, 2)));
        }
        let hand: Vec<_> = if seat == 0 {
            (0..3)
                .map(|n| w15a_initiator(&format!("Initiative {n}")))
                .chain([w11d_venturer("Venture")])
                .collect()
        } else {
            vec![w15a_initiator(&format!("Seat Initiative {seat}"))]
        };
        scenario = scenario.hand(seat, hand).battlefield(seat, permanents);
    }
    scenario
}

/// Answers passively until the stack and the triggers are empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
        driver.answer_passively()?;
    }
    Ok(())
}

fn ventures(driver: &Driver) -> Vec<(usize, String, String)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Ventured(v) => Some((v.player.index(), v.dungeon.clone(), v.room_name.clone())),
            _ => None,
        })
        .collect()
}

/// Whether a triggered ability is on the stack.
fn trigger_on_stack(driver: &Driver) -> bool {
    let state = driver.state();
    state.stack().iter().any(|id| {
        state
            .object(*id)
            .is_some_and(|o| o.kind() == mtg_engine::object::ObjectKind::Ability)
    })
}

fn holder(driver: &Driver) -> Option<usize> {
    driver
        .state()
        .w15a_initiative()
        .map(mtg_core::ids::PlayerId::index)
}

#[rstest]
fn taking_the_initiative_ventures_into_undercity_and_again_on_a_second_take(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Initiative 0")?;
    settle(&mut driver)?;
    assert_eq!(holder(&driver), Some(0));
    assert_eq!(
        ventures(&driver),
        [(0, "Undercity".to_owned(), "Secret Entrance".to_owned())]
    );
    // The Venture cast in response to the second take's trigger resolves first, also into Undercity's next room.
    driver.cast("Initiative 1")?;
    while !trigger_on_stack(&driver) {
        driver.answer_passively()?;
    }
    driver.cast("Venture")?;
    settle(&mut driver)?;
    let walked = ventures(&driver);
    assert_eq!(walked.len(), 3, "{walked:?}");
    assert!(walked.iter().all(|v| v.1 == "Undercity"));
    assert_eq!(holder(&driver), Some(0));
    assert_eq!(
        driver.state().zone(Zone::Command(pid(0))).len(),
        2,
        "Undercity and the initiative"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn combat_damage_takes_the_initiative_and_the_holder_ventures_at_their_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Initiative 0")?;
    settle(&mut driver)?;
    driver.idle_until(2, Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 0)])?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(holder(&driver), Some(1));
    assert!(
        driver.state().zone(Zone::Command(pid(0))).len() == 1,
        "only seat 0's Undercity is left"
    );
    let seat1 = ventures(&driver).iter().filter(|v| v.0 == 1).count();
    assert_eq!(seat1, 1, "taking it in combat ventured");
    driver.idle_until(2 + u32::try_from(seats).unwrap(), Step::Main1)?;
    let seat1 = ventures(&driver).iter().filter(|v| v.0 == 1).count();
    assert_eq!(seat1, 2, "the holder ventured at their upkeep");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn a_holder_who_leaves_passes_the_initiative_to_the_active_player(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.idle_until(2, Step::Main1)?;
    driver.cast("Seat Initiative 1")?;
    settle(&mut driver)?;
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(holder(&driver), Some(1));
    driver.concede(pid(1))?;
    settle(&mut driver)?;
    assert_eq!(holder(&driver), Some(2));
    assert!(ventures(&driver).iter().any(|v| v.0 == 2));
    Ok(())
}

#[rstest]
fn the_holder_walks_undercity_to_its_completion(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    for n in 0..3 {
        driver.cast(&format!("Initiative {n}"))?;
        settle(&mut driver)?;
    }
    driver.cast("Venture")?;
    settle(&mut driver)?;
    // Four rooms down, one each upkeep of seat 0 finishes the walk to Throne of the Dead Three.
    let mut turn = 1;
    while driver.state().w11d_completed(pid(0)) == 0 && turn < 40 {
        turn += 1;
        driver.idle_until(turn, Step::Main1)?;
        settle(&mut driver)?;
    }
    let rooms: Vec<String> = ventures(&driver)
        .into_iter()
        .filter(|v| v.0 == 0)
        .map(|v| v.2)
        .collect();
    assert_eq!(rooms.first().map(String::as_str), Some("Secret Entrance"));
    assert!(
        rooms.iter().any(|room| room == "Throne of the Dead Three"),
        "{rooms:?}"
    );
    assert_eq!(driver.state().w11d_completed(pid(0)), 1);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
