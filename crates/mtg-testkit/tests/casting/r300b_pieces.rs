//! Goal: the r30-0b engine pieces play as their rules say at tables of two to four seats: mana whose rider counts the commander's casts from the command zone (Opal Palace's counters, Study Hall's amount) counts that commander alone, this cast included, after zero to two earlier casts, and gives a spell that is not the commander nothing; an ability only the owner may activate is offered to its owner whoever controls the permanent, and to nobody else.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const LAND: &str = "Rider Land";

/// Opal Palace's rider: counters for each cast of the commander from the command zone.
const COUNTERS: &str =
    r#"{"only":[{"Enters":{"spell":{"facts":["YourCommander"]},"per_command_cast":true}}]}"#;

/// Study Hall's amount read by a rider that gains that much life.
const LIFE: &str = r#"{"when_spent":{"spell":{"facts":["YourCommander"]},"effects":[{"GainLife":{"amount":{"CastsFromCommand":"Triggering"}}}]}}"#;

/// A land that taps for {G} with the rider fields `rider`.
fn land(rider: &str) -> Arc<CardDefinition> {
    let rider = rider
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .unwrap();
    let rules = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["Green"]}},{rider}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(LAND)
            .with_type(CardType::Land)
            .with_rules(serde_json::from_str(&rules).unwrap()),
    )
}

/// A green 2/2 legendary commander for {G}.
fn commander() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Cmd")
            .with_mana_cost(ManaCost::parse("{G}").unwrap())
            .with_supertype(Supertype::Legendary)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2),
    )
}

/// Seat 0 has the commander, the rider land, Forests for the tax and a green Bear in hand.
fn table(seats: usize, rider: &str) -> Scenario {
    let mut lands = vec![ready(land(rider))];
    lands.extend((0..10).map(|_| ready(forest())));
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, vec![commander()])
        .hand(0, [creature("Bear", 0, &[Color::Green], 2, 2)])
        .battlefield(0, lands)
}

fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("the stack never emptied");
}

/// Casts the commander `earlier` times paid by Forests and returns it to the command zone each time.
fn earlier_casts(driver: &mut Driver, earlier: u32) -> Result<(), ScriptError> {
    for done in 0..earlier {
        let taps = vec!["Forest"; 1 + 2 * done as usize];
        driver.cast_tapping("Cmd", &taps)?;
        resolve(driver)?;
        let id = driver.id_of("Cmd")?;
        driver.manual(ManualAction::Move {
            object: id,
            to: ManualZone::Graveyard,
        })?;
        assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
        driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        assert_eq!(
            driver.state().object(id).unwrap().zone(),
            Zone::Command(pid(0))
        );
    }
    Ok(())
}

/// Casts the commander once more, its {G} paid by the rider land and its tax by Forests.
fn cast_with_rider(driver: &mut Driver, earlier: u32) -> Result<(), ScriptError> {
    let mut taps = vec![LAND];
    taps.extend(vec!["Forest"; 2 * earlier as usize]);
    driver.cast_tapping("Cmd", &taps)?;
    resolve(driver)
}

fn plus_counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .expect("on the battlefield")
        .counters()
        .filter(|(kind, _)| **kind == CounterKind::PlusOnePlusOne)
        .map(|(_, n)| n)
        .sum()
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

#[rstest]
fn the_commander_enters_with_a_counter_for_each_cast_this_one_included(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] earlier: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, COUNTERS))?;
    earlier_casts(&mut driver, earlier)?;
    cast_with_rider(&mut driver, earlier)?;
    assert_eq!(
        driver.state().commander_casts(driver.id_of("Cmd")?),
        earlier + 1
    );
    assert_eq!(plus_counters(&driver, "Cmd"), earlier + 1);
    Ok(())
}

#[rstest]
fn the_amount_counts_the_casts_of_that_commander(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] earlier: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, LIFE))?;
    earlier_casts(&mut driver, earlier)?;
    let before = life(&driver);
    cast_with_rider(&mut driver, earlier)?;
    assert_eq!(life(&driver) - before, i32::try_from(earlier).unwrap() + 1);
    Ok(())
}

#[rstest]
#[case::counters(COUNTERS)]
#[case::life(LIFE)]
fn a_spell_that_is_not_the_commander_gets_nothing(
    #[case] rider: &str,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, rider))?;
    let before = life(&driver);
    driver.cast_tapping("Bear", &[LAND])?;
    resolve(&mut driver)?;
    assert_eq!(plus_counters(&driver, "Bear"), 0);
    assert_eq!(life(&driver), before);
    Ok(())
}

/// Whether the deciding player is offered the first ability of the named permanent.
fn offered(driver: &Driver, name: &str) -> bool {
    driver.offered_abilities(name).unwrap().contains(&0)
}

/// Passes until the seat `seat` decides.
fn pass_to(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..20 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    panic!("seat {seat} never decided");
}

#[rstest]
#[case::the_owner_alone("Personal Incarnation", [true, true, false])]
#[case::the_controller("Nomads en-Kor", [true, false, true])]
fn an_owner_only_ability_follows_the_owner_not_the_controller(
    #[case] name: &str,
    #[case] expected: [bool; 3],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = Scenario::new(seats).battlefield(0, [ready(corpus::card(name).unwrap())]);
    let mut driver = Driver::from_scenario(table)?;
    pass_to(&mut driver, 0)?;
    assert_eq!(offered(&driver, name), expected[0], "owner and controller");
    let id = driver.id_of(name)?;
    driver.add_effect(
        driver.on(id),
        Modification::SetController(pid(1)),
        Duration::Forever,
    )?;
    assert_eq!(
        offered(&driver, name),
        expected[1],
        "the owner, not controlling it"
    );
    pass_to(&mut driver, 1)?;
    assert_eq!(
        offered(&driver, name),
        expected[2],
        "the controller, not owning it"
    );
    if seats > 2 {
        pass_to(&mut driver, 2)?;
        assert!(!offered(&driver, name), "neither owner nor controller");
    }
    Ok(())
}
