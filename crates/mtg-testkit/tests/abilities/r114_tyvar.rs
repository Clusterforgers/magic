//! Goal: Tyvar the Bellicose at 2 to 4 seats: a mana ability of a creature its controller controls resolves as it is
//! activated (CR 605.3a), and the creature gets as many +1/+1 counters as the mana it produced, once each turn; a
//! creature of an opponent gets none. Elves that attack together gain deathtouch until end of turn, other attackers do
//! not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A 1/1 creature of `subtype` with the mana ability `cost: Add mana`.
fn druid(name: &str, subtype: &str, ability: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(ability).unwrap()),
    )
}

const TWO_GREEN: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green","Green"]}}}]}}"#;
const LIFE_FOR_GREEN: &str = r#"{"Activated":{"cost":[{"PayLife":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#;

/// Seat 0 controls Tyvar, a Twin Druid ({T}: Add {G}{G}), a Life Druid (Pay 1 life: Add {G}) and a non-Elf Bear; seat 1
/// controls a Twin Druid of its own.
fn tyvar(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Tyvar the Bellicose").unwrap()),
                ready(druid("Twin Druid", "Elf", TWO_GREEN)),
                ready(druid("Life Druid", "Elf", LIFE_FOR_GREEN)),
                ready(druid("Bear", "Bear", LIFE_FOR_GREEN)),
            ],
        )
        .battlefield(1, [ready(druid("Their Druid", "Elf", TWO_GREEN))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

/// +1/+1 counters on the named permanent.
fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

/// Resolves what is on the stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack does not settle".into()))
}

#[rstest]
fn r114_a_mana_ability_puts_the_mana_made_as_counters_once_each_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = tyvar(seats)?;
    driver.tap_for_mana("Twin Druid")?;
    settle(&mut driver)?;
    assert_eq!(counters(&driver, "Twin Druid"), 2, "two mana made");
    driver.tap_for_mana("Life Druid")?;
    settle(&mut driver)?;
    driver.tap_for_mana("Life Druid")?;
    settle(&mut driver)?;
    assert_eq!(counters(&driver, "Life Druid"), 1, "once each turn");
    assert_eq!(counters(&driver, "Their Druid"), 0);
    Ok(())
}

#[rstest]
fn r114_attacking_elves_gain_deathtouch(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = tyvar(seats)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Twin Druid", 1), ("Life Druid", 1), ("Bear", 1)])?;
    settle(&mut driver)?;
    let has = |name: &str| {
        keywords_of(driver.state(), driver.id_of(name).unwrap()).contains(&&Keyword::Deathtouch)
    };
    assert!(has("Twin Druid") && has("Life Druid"));
    assert!(!has("Bear"));
    Ok(())
}
