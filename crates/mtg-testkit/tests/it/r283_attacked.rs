//! Goal: "Cast this spell only during the declare attackers step and only if you've been attacked this step" (Heavy Fog)
//! opens to a player a creature attacks and to no one else, and the fog then prevents the damage the attackers would deal
//! to that player; "Cast this spell only after combat" (Jabari's Influence) opens after the combat phase, not before it,
//! and takes an attacker with a -1/-0 counter, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::casting::lands;
use common::{name_of, pid};
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The cards the holder of priority may cast now.
fn castable(driver: &Driver) -> Vec<String> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return Vec::new();
    };
    options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect()
}

#[rstest]
#[case::the_attacked_player_may_fog(true)]
#[case::a_player_nobody_attacks_may_not(false)]
fn r283_heavy_fog_opens_only_to_an_attacked_player(
    #[case] attacked: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 1 attacks; seat 0 holds the fog. With two seats the only defender is seat 0.
    if !attacked && seats == 2 {
        return Ok(());
    }
    let defender = if attacked { 0 } else { seats - 1 };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 1, Step::DeclareAttackers)
            .battlefield(1, [ready(free_creature("Raider", 3, 3))])
            .battlefield(0, lands(2, forest))
            .hand(0, [corpus().unwrap().card("Heavy Fog").unwrap()]),
    )?;
    driver.attack(&[("Raider", defender)])?;
    for _ in 0..8 {
        if driver.decision()?.player == pid(0) {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(driver.state().turn().step(), Step::DeclareAttackers);
    let offered = castable(&driver);
    assert_eq!(
        offered.contains(&"Heavy Fog".to_owned()),
        attacked,
        "{offered:?}"
    );
    if attacked {
        driver.cast("Heavy Fog")?;
        driver.pass_until(Step::Main2)?;
        let life = driver.state().player(pid(0)).unwrap().life();
        assert_eq!(life, 20, "the attacker's damage is prevented");
    }
    Ok(())
}

/// Passes until seat 0 holds priority in `step`.
fn r283_priority_in(driver: &mut Driver, step: Step) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().turn().step() == step && driver.decision()?.player == pid(0) {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck(format!("seat 0 priority in {step:?}")))
}

#[rstest]
fn r283_jabaris_influence_waits_for_the_end_of_combat_and_takes_an_attacker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 1, Step::Main1)
            .battlefield(1, [ready(free_creature("Raider", 3, 9))])
            .battlefield(0, {
                let mut permanents = lands(5, mtg_testkit::cards::plains);
                permanents.push(ready(free_creature("Wall", 0, 9)));
                permanents
            })
            .hand(0, [corpus().unwrap().card("Jabari's Influence").unwrap()]),
    )?;
    r283_priority_in(&mut driver, Step::Main1)?;
    assert!(
        !castable(&driver).contains(&"Jabari's Influence".to_owned()),
        "not before combat"
    );
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 0)])?;
    r283_priority_in(&mut driver, Step::Main2)?;
    assert!(
        castable(&driver).contains(&"Jabari's Influence".to_owned()),
        "after combat"
    );
    let raider = driver.target_named("Raider")?;
    driver.cast_at("Jabari's Influence", &[raider])?;
    for _ in 0..20 {
        let gone = common::names(driver.state(), mtg_engine::zones::Zone::Graveyard(pid(0)));
        if gone.contains(&"Jabari's Influence".to_owned()) {
            break;
        }
        driver.answer_passively()?;
    }
    let id = driver.id_of("Raider")?;
    assert_eq!(
        mtg_engine::characteristics::controller_of(driver.state(), id),
        Some(pid(0))
    );
    let power =
        mtg_engine::characteristics::characteristics(driver.state(), id).and_then(|c| c.power);
    assert_eq!(power, Some(2), "the -1/-0 counter lowers its power");
    let wall = driver.target_named("Wall")?;
    assert_ne!(raider, wall);
    Ok(())
}
