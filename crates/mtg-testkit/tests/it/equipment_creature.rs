//! Goal: an Equipment that is also a creature is attached to nothing (CR 301.5c, 704.5n), at tables of two, three and four seats: one that becomes a creature while attached comes off in the pass of state-based actions that follows the change (whatever animates it and for as long as it lasts), keeps its place on the battlefield and is attached again by an equip ability once it is no creature; one that is a creature already stays unattached when an equip ability resolves, while a creature Equipment is still a creature another Equipment may equip; an Equipment with reconfigure is no creature while it is attached and stays there, and comes off when an effect makes it a creature while attached; an Aura that is a creature, or on what is no longer a creature it may enchant, is the existing rule and goes to the graveyard; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::attach_cards::{iron_blade, mighty_aura};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
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

fn is_creature(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).is_some_and(|chars| chars.is_creature())
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver.id_of(name).is_ok_and(|id| {
        driver
            .state()
            .object(id)
            .is_some_and(|o| o.zone() == Zone::Battlefield)
    })
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn count(driver: &Driver, wanted: fn(&Event) -> bool) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| wanted(event))
        .count()
}

fn unattachments(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::Unattached { .. }))
}

fn attachments(driver: &Driver) -> usize {
    count(driver, |event| matches!(event, Event::Attached { .. }))
}

fn bonesplitter() -> Arc<CardDefinition> {
    card("Bonesplitter").unwrap()
}

/// Seat 0 has `equipment` in play (each attached to the Wolf when `attached`), then a Wolf (3/3), an Elk (1/4) and `mana`
/// Forests; seat 1 has an Ogre (3/4).
fn table(seats: usize, equipment: &[Arc<CardDefinition>], attached: bool, mana: usize) -> Scenario {
    let wolf = equipment.len();
    let mut permanents: Vec<_> = equipment
        .iter()
        .cloned()
        .map(|card| {
            if attached {
                attached_to(ready(card), 0, wolf)
            } else {
                ready(card)
            }
        })
        .collect();
    permanents.extend([
        ready(free_creature("Wolf", 3, 3)),
        ready(free_creature("Elk", 1, 4)),
    ]);
    permanents.extend((0..mana).map(|_| ready(forest())));
    Scenario::new(seats)
        .battlefield(0, permanents)
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))])
}

/// Makes `name` a 5/5 creature the way a planeswalker ultimate does; the size comes first so that no pass finds a creature
/// with no toughness.
fn animate(driver: &mut Driver, name: &str, duration: Duration) -> Result<(), ScriptError> {
    let id = driver.id_of(name)?;
    driver.add_effect(
        driver.on(id),
        Modification::SetPowerToughness {
            power: 5,
            toughness: 5,
        },
        duration,
    )?;
    driver.add_effect(
        driver.on(id),
        Modification::AddType(CardType::Creature),
        duration,
    )?;
    Ok(())
}

/// Activates the first ability of `source` at `target`, pays for it and lets it resolve.
fn equip(driver: &mut Driver, source: &str, target: &str) -> Result<(), ScriptError> {
    driver.begin_activation(source)?;
    let target = driver.target_named(target)?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(driver)
}

// An Equipment that becomes a creature while attached

#[rstest]
fn an_equipment_that_becomes_a_creature_while_attached_comes_off_in_the_next_pass(
    #[values(2, 3, 4)] seats: usize,
    #[values(bonesplitter(), iron_blade().unwrap())] card: Arc<CardDefinition>,
    #[values(Duration::UntilEndOfTurn, Duration::Forever)] duration: Duration,
) -> Result<(), ScriptError> {
    let name = card.name.clone();
    let scenario = table(seats, &[card], true, 2);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert_eq!(host_name(&driver, &name).as_deref(), Some("Wolf"));
    assert_eq!(unattachments(&driver), 0);
    animate(&mut driver, &name, duration)?;
    assert!(is_creature(&driver, &name));
    assert!(on_battlefield(&driver, &name));
    assert_eq!(host_name(&driver, &name), None);
    assert_eq!(unattachments(&driver), 1);
    // Nothing attaches it again and nothing comes off a second time while the table plays on.
    driver.pass()?;
    assert_eq!(host_name(&driver, &name), None);
    assert_eq!(unattachments(&driver), 1);
    replays(&driver, &scenario.setup()?);
    Ok(())
}

#[rstest]
fn the_equipment_that_came_off_gives_its_bonus_to_nobody(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[bonesplitter()], true, 2))?;
    let wolf = driver.id_of("Wolf")?;
    let power = |driver: &Driver| {
        characteristics(driver.state(), wolf)
            .unwrap()
            .power
            .unwrap()
    };
    assert_eq!(power(&driver), 5);
    animate(&mut driver, "Bonesplitter", Duration::Forever)?;
    assert_eq!(power(&driver), 3);
    Ok(())
}

#[rstest]
fn two_equipment_that_become_creatures_together_come_off_in_one_pass(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        &[bonesplitter(), iron_blade().unwrap()],
        true,
        2,
    ))?;
    animate(&mut driver, "Bonesplitter", Duration::Forever)?;
    assert_eq!(host_name(&driver, "Bonesplitter"), None);
    assert_eq!(host_name(&driver, "Iron Blade").as_deref(), Some("Wolf"));
    animate(&mut driver, "Iron Blade", Duration::Forever)?;
    assert_eq!(host_name(&driver, "Iron Blade"), None);
    assert_eq!(unattachments(&driver), 2);
    Ok(())
}

#[rstest]
fn an_equipment_comes_off_when_it_is_a_creature_and_the_wolf_is_not(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[bonesplitter()], true, 2))?;
    let wolf = driver.id_of("Wolf")?;
    driver.add_effect(
        driver.on(wolf),
        Modification::RemoveType(CardType::Creature),
        Duration::Forever,
    )?;
    animate(&mut driver, "Bonesplitter", Duration::Forever)?;
    assert_eq!(host_name(&driver, "Bonesplitter"), None);
    assert_eq!(unattachments(&driver), 1, "it comes off once");
    Ok(())
}

// An Equipment that is a creature already

#[rstest]
fn an_equip_ability_does_not_attach_an_equipment_that_is_a_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &[bonesplitter()], false, 2);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    animate(&mut driver, "Bonesplitter", Duration::UntilEndOfTurn)?;
    if !driver.offered_abilities("Bonesplitter")?.is_empty() {
        equip(&mut driver, "Bonesplitter", "Wolf")?;
    }
    assert_eq!(host_name(&driver, "Bonesplitter"), None);
    assert_eq!(attachments(&driver), 0);
    assert!(on_battlefield(&driver, "Bonesplitter"));
    replays(&driver, &scenario.setup()?);
    Ok(())
}

#[rstest]
fn an_equipment_that_was_a_creature_equips_again_once_it_is_none(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[bonesplitter()], true, 3))?;
    let splitter = driver.id_of("Bonesplitter")?;
    animate(&mut driver, "Bonesplitter", Duration::UntilEndOfTurn)?;
    assert_eq!(host_name(&driver, "Bonesplitter"), None);
    driver.add_effect(
        driver.on(splitter),
        Modification::RemoveType(CardType::Creature),
        Duration::Forever,
    )?;
    assert!(!is_creature(&driver, "Bonesplitter"));
    equip(&mut driver, "Bonesplitter", "Wolf")?;
    assert_eq!(
        host_name(&driver, "Bonesplitter").as_deref(),
        Some("Wolf"),
        "it is an Equipment that is no creature"
    );
    assert_eq!(unattachments(&driver), 1);
    Ok(())
}

#[rstest]
fn an_equipment_equips_a_creature_that_is_an_equipment_itself(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        &[bonesplitter(), iron_blade().unwrap()],
        false,
        3,
    ))?;
    animate(&mut driver, "Iron Blade", Duration::Forever)?;
    equip(&mut driver, "Bonesplitter", "Iron Blade")?;
    assert_eq!(
        host_name(&driver, "Bonesplitter").as_deref(),
        Some("Iron Blade")
    );
    driver.pass()?;
    assert_eq!(
        host_name(&driver, "Bonesplitter").as_deref(),
        Some("Iron Blade")
    );
    assert_eq!(unattachments(&driver), 0);
    Ok(())
}

// Reconfigure

#[rstest]
fn an_equipment_with_reconfigure_is_no_creature_while_attached_and_stays_there(
    #[values(2, 3, 4)] seats: usize,
    #[values("Lizard Blades", "Bronzeplate Boar", "Armguard Familiar")] name: &str,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &[card(name).unwrap()], true, 3);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    assert_eq!(host_name(&driver, name).as_deref(), Some("Wolf"));
    assert!(!is_creature(&driver, name));
    driver.pass()?;
    assert_eq!(host_name(&driver, name).as_deref(), Some("Wolf"));
    assert!(!is_creature(&driver, name));
    assert_eq!(unattachments(&driver), 0);
    replays(&driver, &scenario.setup()?);
    Ok(())
}

#[rstest]
fn an_equipment_with_reconfigure_that_is_a_creature_again_while_attached_comes_off(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(table(seats, &[card("Lizard Blades").unwrap()], true, 3))?;
    assert_eq!(host_name(&driver, "Lizard Blades").as_deref(), Some("Wolf"));
    animate(&mut driver, "Lizard Blades", Duration::Forever)?;
    assert!(is_creature(&driver, "Lizard Blades"));
    assert_eq!(host_name(&driver, "Lizard Blades"), None);
    assert_eq!(unattachments(&driver), 1);
    Ok(())
}

// An Aura

#[rstest]
#[case::the_aura_becomes_a_creature(true)]
#[case::its_host_becomes_a_land(false)]
fn an_aura_goes_to_the_graveyard_when_it_is_a_creature_or_its_host_is_no_creature_any_more(
    #[values(2, 3, 4)] seats: usize,
    #[case] aura_changes: bool,
) -> Result<(), ScriptError> {
    let aura = mighty_aura().unwrap();
    let name = aura.name.clone();
    let mut driver = Driver::from_scenario(table(seats, &[aura], true, 0))?;
    assert_eq!(host_name(&driver, &name).as_deref(), Some("Wolf"));
    if aura_changes {
        animate(&mut driver, &name, Duration::Forever)?;
    } else {
        let wolf = driver.id_of("Wolf")?;
        driver.add_effect(
            driver.on(wolf),
            Modification::RemoveType(CardType::Creature),
            Duration::Forever,
        )?;
        driver.add_effect(
            driver.on(wolf),
            Modification::AddType(CardType::Land),
            Duration::Forever,
        )?;
    }
    let graveyard = common::names(driver.state(), Zone::Graveyard(common::pid(0)));
    assert_eq!(graveyard, std::slice::from_ref(&name));
    assert!(!on_battlefield(&driver, &name));
    assert_eq!(unattachments(&driver), 0, "an Aura is not unattached");
    Ok(())
}
