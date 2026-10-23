//! Goal: For Mirrodin! and job select play as the rules say, at tables of two, three and four seats: the Equipment enters, its trigger creates a 2/2 red Rebel or a 1/1 colorless Hero creature token under its controller's control and attaches the Equipment to it within one resolution, so the token never stands without the bonus at a decision; the bonus, keywords and creature type of the Equipment apply to the token; when the token dies the Equipment stays and equips another creature; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::cards::{burn, free_creature, mountain};
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

fn keyword_names(driver: &Driver, name: &str) -> Vec<&'static str> {
    let id = driver.id_of(name).unwrap();
    keywords_of(driver.state(), id)
        .iter()
        .map(|held| held.name())
        .collect()
}

fn tokens_named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| name_of(driver, *id) == name)
        .collect()
}

fn subtypes_of(driver: &Driver, name: &str) -> Vec<String> {
    let id = driver.id_of(name).unwrap();
    let given = characteristics(driver.state(), id).unwrap().subtypes;
    let printed = driver.state().object(id).unwrap().card().subtypes.clone();
    printed.iter().chain(&given).map(|s| s.0.clone()).collect()
}

/// Seat 0 holds the Equipment and a Blast and has a Bear and ten Mountains.
fn table(seats: usize, equipment: &str) -> Scenario {
    let mut permanents = vec![ready(free_creature("Grizzly Bears", 2, 2))];
    permanents.extend((0..10).map(|_| ready(mountain())));
    Scenario::new(seats)
        .hand(0, [card(equipment).unwrap(), burn("Blast", 4)])
        .battlefield(0, permanents)
}

/// Casts the Equipment and lets it and its trigger resolve.
fn enter(driver: &mut Driver, equipment: &str) -> Result<(), ScriptError> {
    driver.cast(equipment)?;
    settle(driver)
}

#[rstest]
#[case::for_mirrodin("Vulshok Splitter", "Rebel", &[Color::Red], (4, 2), &["Rebel"], &[])]
#[case::for_mirrodin_with_a_penalty("Barbed Batterfist", "Rebel", &[Color::Red], (3, 1), &["Rebel"], &[])]
#[case::for_mirrodin_with_keywords("Dragonwing Glider", "Rebel", &[Color::Red], (4, 4), &["Rebel"], &["Flying", "Haste"])]
#[case::job_select("Monk's Fist", "Hero", &[], (2, 1), &["Hero", "Monk"], &[])]
#[case::job_select_with_a_bonus("Warrior's Sword", "Hero", &[], (4, 3), &["Hero", "Warrior"], &[])]
#[case::job_select_with_keywords("Samurai's Katana", "Hero", &[], (3, 3), &["Hero", "Samurai"], &["Trample", "Haste"])]
fn the_equipment_enters_with_a_token_it_is_attached_to(
    #[case] equipment: &str,
    #[case] token: &str,
    #[case] colors: &[Color],
    #[case] now: (i32, i32),
    #[case] subtypes: &[&str],
    #[case] keywords: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, equipment);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let begun = driver.engine().events().len();
    enter(&mut driver, equipment)?;
    let [made_token] = tokens_named(&driver, token)[..] else {
        panic!("one {token}");
    };
    assert_eq!(host_name(&driver, equipment).as_deref(), Some(token));
    assert_eq!(stats(&driver, token), now);
    assert_eq!(subtypes_of(&driver, token), subtypes);
    let held = keyword_names(&driver, token);
    assert!(keywords.iter().all(|word| held.contains(word)), "{held:?}");
    assert_eq!(
        controller_of(driver.state(), made_token),
        Some(common::pid(0))
    );
    let object = driver.state().object(made_token).unwrap();
    assert!(object.base_types().contains(CardType::Creature));
    assert_eq!(object.card().colors().len() as usize, colors.len());
    assert!(
        colors
            .iter()
            .all(|color| object.card().colors().contains(*color))
    );
    // The token is made and the Equipment attached to it in the one resolution.
    let events = &driver.engine().events()[begun..];
    let made = events
        .iter()
        .position(|e| matches!(e, Event::TokenCreated(created) if created.object == made_token))
        .unwrap();
    let attached = events
        .iter()
        .position(|e| matches!(e, Event::Attached { to, .. } if *to == made_token))
        .unwrap();
    assert!(made < attached);
    assert!(
        !events[made..attached].iter().any(|e| matches!(
            e,
            Event::PriorityGranted { .. } | Event::PriorityPassed { .. }
        )),
        "nobody receives priority between the token and the attachment"
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::for_mirrodin("Vulshok Splitter", "Rebel", (4, 2))]
#[case::job_select("Warrior's Sword", "Hero", (5, 4))]
fn when_the_token_dies_the_equipment_stays_and_equips_another_creature(
    #[case] equipment: &str,
    #[case] token: &str,
    #[case] bear: (i32, i32),
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, equipment))?;
    enter(&mut driver, equipment)?;
    let target = driver.target_named(token)?;
    driver.cast_at("Blast", &[target])?;
    settle(&mut driver)?;
    assert!(tokens_named(&driver, token).is_empty(), "the token died");
    assert_eq!(host_name(&driver, equipment), None);
    driver.begin_ability(equipment, 0)?;
    let chosen = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[chosen])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(
        host_name(&driver, equipment).as_deref(),
        Some("Grizzly Bears")
    );
    assert_eq!(stats(&driver, "Grizzly Bears"), bear);
    Ok(())
}
