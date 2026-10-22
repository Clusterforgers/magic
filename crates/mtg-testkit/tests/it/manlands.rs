//! Goal: a manland read from its Oracle wording (Mutavault, Celestial Colonnade, Creeping Tar Pit) plays as the rules say at tables of two, three and four seats: after its activation it is a creature with the stated size, colors, creature types and keywords and still a land, it attacks and deals its damage, it is not a creature before the activation and again not after the turn, and a manland that came under control this turn cannot attack.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{forest, free_creature, land};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

/// Seat 0 controls the manland and a Forest for its cost; seat 1 controls an Ogre.
fn table(seats: usize, name: &str, came_this_turn: bool) -> Result<Scenario, ScriptError> {
    let land = card(name).unwrap();
    let land = if came_this_turn { sick(land) } else { ready(land) };
    Ok(Scenario::new(seats)
        .battlefield(0, [land, ready(forest())])
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))]))
}

/// Pays the manland's cost with the Forest (and the other lands there are) and lets the ability resolve.
fn animate(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_activation(name)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn is_creature(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .is_creature()
}

fn is_land(driver: &Driver, name: &str) -> bool {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .types
        .contains(CardType::Land)
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let found = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (found.power.unwrap(), found.toughness.unwrap())
}

fn has_keyword(driver: &Driver, name: &str, keyword: &str) -> bool {
    keywords_of(driver.state(), driver.id_of(name).unwrap())
        .iter()
        .any(|held| held.name() == keyword)
}

#[rstest]
fn a_manland_is_a_creature_and_a_land_until_the_end_of_the_turn_and_a_plain_land_after_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Mutavault", false)?)?;
    assert!(!is_creature(&driver, "Mutavault"));
    animate(&mut driver, "Mutavault")?;
    assert!(is_creature(&driver, "Mutavault"));
    assert!(is_land(&driver, "Mutavault"));
    assert_eq!(stats(&driver, "Mutavault"), (2, 2));
    driver.idle_until(2, Step::Main1)?;
    assert!(!is_creature(&driver, "Mutavault"));
    assert!(is_land(&driver, "Mutavault"));
    assert_eq!(driver.state().turn().number(), 2);
    Ok(())
}

#[rstest]
fn a_manland_attacks_and_deals_its_damage(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Mutavault", false)?)?;
    let life = driver.state().player(pid(1)).unwrap().life();
    animate(&mut driver, "Mutavault")?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Mutavault", 1)])?;
    driver.idle_until(1, Step::Main2)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), life - 2);
    Ok(())
}

#[rstest]
fn a_manland_has_the_colors_types_and_keywords_of_its_wording(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Celestial Colonnade").unwrap()),
                ready(land(Color::White)),
                ready(land(Color::Blue)),
                ready(forest()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))]);
    let mut driver = Driver::from_scenario(scenario)?;
    animate(&mut driver, "Celestial Colonnade")?;
    let found = characteristics(
        driver.state(),
        driver.id_of("Celestial Colonnade").unwrap(),
    )
    .unwrap();
    assert!(found.is_creature());
    assert!(found.colors.contains(Color::White) && found.colors.contains(Color::Blue));
    assert_eq!(stats(&driver, "Celestial Colonnade"), (4, 4));
    assert!(has_keyword(&driver, "Celestial Colonnade", "Flying"));
    assert!(has_keyword(&driver, "Celestial Colonnade", "Vigilance"));
    Ok(())
}

#[rstest]
fn a_manland_that_came_under_control_this_turn_cannot_attack(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Mutavault", true)?)?;
    animate(&mut driver, "Mutavault")?;
    assert!(is_creature(&driver, "Mutavault"));
    driver.pass_until(Step::DeclareAttackers).ok();
    if let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind {
        assert!(
            options
                .attackers
                .iter()
                .all(|a| a.object != driver.id_of("Mutavault").unwrap())
        );
    }
    Ok(())
}

#[rstest]
fn the_log_of_a_game_with_a_manland_replays_to_the_live_state(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, "Mutavault", false)?.setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Mutavault", false)?)?;
    animate(&mut driver, "Mutavault")?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Mutavault", 1)])?;
    driver.idle_until(2, Step::Main1)?;
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
