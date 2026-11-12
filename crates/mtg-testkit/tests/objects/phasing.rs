//! Goal: phasing plays as CR 702.26 says at tables of two, three and four seats: a spell that phases permanents out takes them and what is attached to them off the board without leaving the battlefield, a life total that cannot change and protection from everything last until the caster's next turn, everything phases in during its controller's untap step before the untap and is the same object with the same state, nothing enters or leaves, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_core::ids::ObjectId;
use mtg_engine::keywords::player_keywords;
use mtg_engine::rules::life_total_cant_change;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, replay};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

/// Seat 0 holds `hand`, controls `lands` Plains, a Bear and a Robe of Stars on the Bear; every other seat has a Wolf.
fn table(seats: usize, hand: &[&str], lands: usize) -> (Scenario, GameSetup) {
    let mut mine: Vec<_> = (0..lands).map(|_| ready(plains())).collect();
    mine.push(ready(free_creature("Bear", 2, 2)));
    mine.push(attached_to(ready(card("Robe of Stars").unwrap()), 0, lands));
    let mut scenario = Scenario::new(seats)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Wolf", 3, 3))]);
    }
    let setup = scenario.clone().setup().unwrap();
    (scenario, setup)
}

fn id(driver: &Driver, name: &str) -> ObjectId {
    // `Driver::id_of` looks on the battlefield, which a permanent that is phased out has left.
    driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap()
}

fn zone_of(driver: &Driver, name: &str) -> Zone {
    driver.state().object(id(driver, name)).unwrap().zone()
}

/// Every seat passes until the stack is empty.
fn resolve(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats * 4 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Whether the seat has protection from everything.
fn protected(driver: &Driver, seat: usize) -> bool {
    player_keywords(driver.state(), pid(seat)).iter().any(
        |keyword| matches!(keyword, mtg_cards::keyword::Keyword::Protection(q) if q.is_empty()),
    )
}

/// The number of the turn on which seat 0 takes its next turn.
fn next_turn(seats: usize) -> u32 {
    u32::try_from(seats).unwrap() + 1
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The events of the log from `from` on.
fn since(driver: &Driver, from: usize) -> Vec<Event> {
    driver.engine().events()[from..].to_vec()
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn teferis_protection_takes_everything_off_the_board_until_the_next_turn(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let (scenario, setup) = table(seats, &["Teferi's Protection"], 3);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Teferi's Protection", &["Plains", "Plains", "Plains"])?;
    resolve(&mut driver, seats)?;
    for name in ["Plains", "Bear", "Robe of Stars"] {
        assert_eq!(zone_of(&driver, name), Zone::PhasedOut, "{name}");
    }
    assert_eq!(zone_of(&driver, "Wolf"), Zone::Battlefield);
    // The spell has resolved and exiled itself: it is not put into the graveyard after.
    assert_eq!(zone_of(&driver, "Teferi's Protection"), Zone::Exile);
    // The permanents that phased out stay objects of the game and appear in no scan of the battlefield.
    assert!(
        !driver
            .state()
            .zone(Zone::Battlefield)
            .contains(&id(&driver, "Bear"))
    );
    assert_eq!(driver.state().zone(Zone::PhasedOut).len(), 5);
    assert!(life_total_cant_change(driver.state(), pid(0)));
    assert!(!life_total_cant_change(driver.state(), pid(1)));
    assert!(protected(&driver, 0));
    assert!(!protected(&driver, 1));
    let before = driver.engine().events().len();
    // Seat 0's next turn: it phases in before the untap, and the two rules have ended.
    driver.idle_until(next_turn(seats), Step::Upkeep)?;
    for name in ["Plains", "Bear", "Robe of Stars"] {
        assert_eq!(zone_of(&driver, name), Zone::Battlefield, "{name}");
    }
    assert!(!life_total_cant_change(driver.state(), pid(0)));
    assert!(!protected(&driver, 0));
    let log = since(&driver, before);
    let first_phase_in = log
        .iter()
        .position(|e| matches!(e, Event::PermanentPhasedIn { .. }))
        .unwrap();
    let first_untap = log
        .iter()
        .position(|e| matches!(e, Event::PermanentUntapped { .. }))
        .unwrap();
    assert!(first_phase_in < first_untap, "phasing in comes first");
    let untapped: Vec<_> = log
        .iter()
        .filter(|e| matches!(e, Event::PermanentUntapped { .. }))
        .collect();
    assert_eq!(
        untapped.len(),
        3,
        "the Plains tapped for the spell untap on phasing in"
    );
    assert!(log.iter().all(|e| !matches!(
        e,
        Event::Entered { .. } | Event::ZoneChanged { .. } | Event::TokenCreated(_)
    )));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn clever_concealment_takes_the_chosen_permanents_and_what_is_attached(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let (scenario, setup) = table(seats, &["Clever Concealment"], 4);
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = driver.target_named("Bear")?;
    driver.begin_cast("Clever Concealment")?;
    driver.choose_targets(&[bear])?;
    driver.pay_tapping(&["Plains", "Plains", "Plains", "Plains"], 0)?;
    resolve(&mut driver, seats)?;
    assert_eq!(zone_of(&driver, "Bear"), Zone::PhasedOut);
    // The Equipment went with it and comes back with it; the Plains and the other seats stay.
    assert_eq!(zone_of(&driver, "Robe of Stars"), Zone::PhasedOut);
    let robe = driver.state().object(id(&driver, "Robe of Stars")).unwrap();
    assert!(robe.permanent().unwrap().phasing().unwrap().indirect);
    assert_eq!(zone_of(&driver, "Wolf"), Zone::Battlefield);
    assert_eq!(zone_of(&driver, "Plains"), Zone::Battlefield);
    assert!(!life_total_cant_change(driver.state(), pid(0)));
    driver.idle_until(next_turn(seats), Step::Upkeep)?;
    assert_eq!(zone_of(&driver, "Bear"), Zone::Battlefield);
    assert_eq!(zone_of(&driver, "Robe of Stars"), Zone::Battlefield);
    let robe = driver.state().object(id(&driver, "Robe of Stars")).unwrap();
    assert_eq!(
        driver.state().host_of(robe.id()),
        Some(id(&driver, "Bear")),
        "it is still attached"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn robe_of_stars_phases_out_its_creature_and_itself_for_two_mana(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let (scenario, setup) = table(seats, &[], 2);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Robe of Stars", 1)?;
    driver.pay_tapping(&["Plains", "Plains"], 0)?;
    resolve(&mut driver, seats)?;
    assert_eq!(zone_of(&driver, "Bear"), Zone::PhasedOut);
    assert_eq!(zone_of(&driver, "Robe of Stars"), Zone::PhasedOut);
    assert_eq!(zone_of(&driver, "Plains"), Zone::Battlefield);
    driver.idle_until(next_turn(seats), Step::Upkeep)?;
    assert_eq!(zone_of(&driver, "Bear"), Zone::Battlefield);
    assert_eq!(zone_of(&driver, "Robe of Stars"), Zone::Battlefield);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn guardian_of_faith_phases_out_the_creatures_chosen_as_it_enters(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let (scenario, setup) = table(seats, &["Guardian of Faith"], 3);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Guardian of Faith", &["Plains", "Plains", "Plains"])?;
    resolve_with_targets(&mut driver, seats, &["Bear"])?;
    assert_eq!(zone_of(&driver, "Bear"), Zone::PhasedOut);
    assert_eq!(zone_of(&driver, "Guardian of Faith"), Zone::Battlefield);
    assert_eq!(zone_of(&driver, "Wolf"), Zone::Battlefield);
    replays(&driver, &setup);
    Ok(())
}

/// Passes until the stack is empty, choosing the named permanents when a trigger of seat 0 asks for targets.
fn resolve_with_targets(
    driver: &mut Driver,
    seats: usize,
    names: &[&str],
) -> Result<(), ScriptError> {
    for _ in 0..seats * 6 {
        if let Some(mtg_engine::decision::DecisionKind::ChooseTargets(_)) =
            driver.engine().decision().map(|d| &d.kind)
        {
            let targets: Vec<_> = names
                .iter()
                .map(|name| driver.target_named(name))
                .collect::<Result<_, _>>()?;
            driver.choose_targets(&targets)?;
        } else if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        } else {
            driver.pass()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}
