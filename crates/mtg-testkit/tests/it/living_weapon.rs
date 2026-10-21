//! Goal: living weapon plays as the rules say, at tables of two, three and four seats: Batterskull cast and resolved enters, its trigger creates a 0/0 black Phyrexian Germ creature token under its controller's control and attaches the Equipment to it within one resolution, so the Germ is a 4/4 with vigilance and lifelink and is never a 0/0 at a decision; when the Germ dies the Equipment stays and equips another creature; equipping another creature takes the bonus off the Germ, which dies as a 0/0 and ceases to exist; returning the Equipment to its owner's hand and casting it again makes a new Germ; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::events::ZoneCause;
use mtg_engine::keywords::keywords_of;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::cards::{burn, forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn name_of(driver: &Driver, id: ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

fn host_name(driver: &Driver, name: &str) -> Option<String> {
    let id = driver.id_of(name).ok()?;
    driver.state().host_of(id).map(|host| name_of(driver, host))
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn has_keyword(driver: &Driver, name: &str, keyword: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    keywords_of(driver.state(), id)
        .iter()
        .any(|held| held.name() == keyword)
}

fn germs(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(driver, *id) == "Phyrexian Germ")
        .collect()
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds Batterskull and a Blast and has a Bear, a Mountain and `forests` Forests.
fn table(seats: usize, forests: usize) -> Scenario {
    let mut permanents = vec![
        ready(free_creature("Grizzly Bears", 2, 2)),
        ready(mountain()),
    ];
    permanents.extend((0..forests).map(|_| ready(forest())));
    Scenario::new(seats)
        .hand(0, [card("Batterskull").unwrap(), burn("Blast", 4)])
        .battlefield(0, permanents)
}

/// Casts Batterskull and lets it and its trigger resolve.
fn enter(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.cast("Batterskull")?;
    settle(driver)
}

#[rstest]
fn the_equipment_enters_with_a_germ_it_is_attached_to(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, 5);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    enter(&mut driver)?;
    let [germ] = germs(&driver)[..] else {
        panic!("one Germ");
    };
    assert_eq!(
        host_name(&driver, "Batterskull").as_deref(),
        Some("Phyrexian Germ")
    );
    assert_eq!(stats(&driver, "Phyrexian Germ"), (4, 4));
    assert!(has_keyword(&driver, "Phyrexian Germ", "Vigilance"));
    assert!(has_keyword(&driver, "Phyrexian Germ", "Lifelink"));
    assert_eq!(controller_of(driver.state(), germ), Some(common::pid(0)));
    // A black creature token with the types of the Germ.
    let object = driver.state().object(germ).unwrap();
    assert_eq!(object.card().colors().len(), 1);
    assert!(object.card().colors().contains(Color::Black));
    assert!(object.base_types().contains(CardType::Creature));
    let subtypes: Vec<&str> = object
        .card()
        .subtypes
        .iter()
        .map(|s| s.0.as_str())
        .collect();
    assert_eq!(subtypes, ["Phyrexian", "Germ"]);
    assert_eq!(
        (object.card().power, object.card().toughness),
        (Some(0), Some(0))
    );
    // The token is made and the Equipment attached to it in the one resolution.
    let events = &driver.engine().events()[begun..];
    let made = events
        .iter()
        .position(|e| matches!(e, Event::TokenCreated(token) if token.object == germ))
        .unwrap();
    let attached = events
        .iter()
        .position(|e| matches!(e, Event::Attached { to, .. } if *to == germ))
        .unwrap();
    assert!(made < attached);
    assert!(
        !events[made..attached].iter().any(|e| matches!(
            e,
            Event::PriorityGranted { .. } | Event::PriorityPassed { .. }
        )),
        "nobody receives priority between the token and the attachment"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn when_the_germ_dies_the_equipment_stays_and_equips_another_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 11))?;
    enter(&mut driver)?;
    let germ = driver.target_named("Phyrexian Germ")?;
    let begun = driver.engine().events().len();
    driver.cast_at("Blast", &[germ])?;
    settle(&mut driver)?;
    assert!(germs(&driver).is_empty(), "the token ceased to exist");
    assert!(driver.engine().events()[begun..].iter().any(|e| matches!(
        e,
        Event::ZoneChanged {
            from: Zone::Battlefield,
            cause: ZoneCause::Destroyed | ZoneCause::StateBased,
            ..
        } | Event::CreatureDied { .. }
    )));
    assert_eq!(host_name(&driver, "Batterskull"), None);
    driver.begin_ability("Batterskull", 0)?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bear])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(
        host_name(&driver, "Batterskull").as_deref(),
        Some("Grizzly Bears")
    );
    assert_eq!(stats(&driver, "Grizzly Bears"), (6, 6));
    Ok(())
}

#[rstest]
fn equipping_another_creature_takes_the_bonus_off_the_germ_which_dies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 10))?;
    enter(&mut driver)?;
    driver.begin_ability("Batterskull", 0)?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bear])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert!(
        germs(&driver).is_empty(),
        "a 0/0 Germ dies once the Equipment leaves it"
    );
    assert_eq!(
        host_name(&driver, "Batterskull").as_deref(),
        Some("Grizzly Bears")
    );
    assert_eq!(stats(&driver, "Grizzly Bears"), (6, 6));
    Ok(())
}

#[rstest]
fn returning_the_equipment_to_hand_and_casting_it_again_makes_a_new_germ(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, 12);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    enter(&mut driver)?;
    let [first] = germs(&driver)[..] else {
        panic!("one Germ");
    };
    // The ability that returns the Equipment is the third rule of the card.
    driver.begin_ability("Batterskull", 2)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert!(
        driver.id_of("Batterskull").is_err(),
        "it is back in its owner's hand"
    );
    assert!(germs(&driver).is_empty(), "and the Germ died as a 0/0");
    enter(&mut driver)?;
    let [second] = germs(&driver)[..] else {
        panic!("a new Germ");
    };
    assert_ne!(first, second);
    assert_eq!(
        host_name(&driver, "Batterskull").as_deref(),
        Some("Phyrexian Germ")
    );
    assert_eq!(stats(&driver, "Phyrexian Germ"), (4, 4));
    replays(&driver, &setup);
    Ok(())
}
