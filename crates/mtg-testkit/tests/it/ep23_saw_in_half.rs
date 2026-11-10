//! Goal: tokens that copy a creature "except their base power is half that creature's power and their base toughness is half that creature's toughness. Round up each time." (Saw in Half) take the halves of the creature's last known power and toughness, its counters included, rounded up; the creature's controller gets them, and an indestructible creature that does not die makes none; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::characteristics;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, instant};
use mtg_testkit::trigger_cards::with_rules;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;
use crate::common::removal::resolve_the_stack;

const SAW: &str = r#"{"Spell":{"effects":[{"Destroy":{"as":"destroyed","what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Bound":"destroyed~moved"},"op":"Ge","right":1}},"then":[{"CreateToken":{"count":2,"token":{"CopyOf":{"except":[{"SetPT":{"power":{"Half":{"of":{"Stat":{"of":{"Bound":"destroyed"},"stat":"Power"}},"up":true}},"toughness":{"Half":{"of":{"Stat":{"of":{"Bound":"destroyed"},"stat":"Toughness"}},"up":true}}}}],"of":{"Bound":"destroyed"}}},"who":{"ControllerOf":{"Target":0}}}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;

fn saw() -> Arc<CardDefinition> {
    with_rules(
        &instant("Saw", 0, &[]),
        vec![serde_json::from_str::<Ability>(SAW).unwrap()],
    )
}

/// The power and toughness of each "Ox" on `seat`'s battlefield.
fn oxen(driver: &Driver, seat: usize) -> Vec<(i32, i32)> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| {
            let object = driver.state().object(*id)?;
            let chars = characteristics(driver.state(), *id)?;
            (object.card().name == "Ox" && object.controller() == pid(seat))
                .then_some((chars.power?, chars.toughness?))
        })
        .collect()
}

#[rstest]
#[case::odd_rounds_up(3, 5, 0, &[(2, 3), (2, 3)])]
#[case::even_halves(4, 2, 0, &[(2, 1), (2, 1)])]
#[case::counters_count(3, 3, 2, &[(3, 3), (3, 3)])]
#[case::zero_stays_zero(0, 1, 0, &[(0, 1), (0, 1)])]
fn ep23_the_copies_take_half_the_creature_s_power_and_toughness(
    #[case] power: i32,
    #[case] toughness: i32,
    #[case] counters: u32,
    #[case] expected: &[(i32, i32)],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ox = with_counters(
        ready(free_creature("Ox", power, toughness)),
        CounterKind::PlusOnePlusOne,
        counters,
    );
    let scenario = Scenario::new(seats).hand(0, [saw()]).battlefield(1, [ox]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Ox")?;
    driver.cast_at("Saw", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(oxen(&driver, 1), expected);
    assert_eq!(oxen(&driver, 0), []);
    Ok(())
}

#[rstest]
fn ep23_a_creature_that_does_not_die_makes_no_copies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ox = Arc::new(
        (*free_creature("Ox", 4, 4))
            .clone()
            .with_rules(Ability::Keyword(Keyword::Indestructible)),
    );
    let scenario = Scenario::new(seats)
        .hand(0, [saw()])
        .battlefield(1, [ready(ox)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Ox")?;
    driver.cast_at("Saw", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(oxen(&driver, 1), [(4, 4)]);
    Ok(())
}
