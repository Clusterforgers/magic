//! Goal: Halfdane's "At the beginning of your upkeep, Halfdane's base power and toughness become equal to the power and toughness of target creature other than Halfdane until the end of your next upkeep." plays at tables of two, three and four seats: the size it takes is the target's as the ability resolves, it lasts through the next upkeep of its controller (so it still holds while the next trigger waits on the stack) and the next trigger's size replaces it, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Halfdane; seat 1 a 5/4 Giant and a 1/6 Wall; the other seats a Bear each.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Halfdane").unwrap())])
        .battlefield(
            1,
            [
                ready(free_creature("Giant", 5, 4)),
                ready(free_creature("Wall", 1, 6)),
            ],
        );
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bear", 2, 2))]);
    }
    scenario
}

fn size(driver: &Driver) -> (i32, i32) {
    let id = driver.id_of("Halfdane").unwrap();
    let face = characteristics(driver.state(), id).unwrap();
    (face.power.unwrap(), face.toughness.unwrap())
}

/// Waits for the upkeep trigger of `turn` to ask its target, then targets `name`; the trigger is on the stack after.
fn target_in_upkeep(driver: &mut Driver, turn: u32, name: &str) -> Result<(), ScriptError> {
    driver.idle_until(turn, Step::Upkeep)?;
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            let target = driver.target_named(name)?;
            driver.choose_targets(&[target])?;
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck(
        "the target of the upkeep trigger".into(),
    ))
}

/// Passes until the stack is empty.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    Ok(())
}

#[rstest]
fn halfdane_takes_the_size_of_its_target_through_its_next_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    assert_eq!(size(&driver), (3, 3));
    let first = 1 + u32::try_from(seats).unwrap();
    target_in_upkeep(&mut driver, first, "Giant")?;
    resolve(&mut driver)?;
    assert_eq!(size(&driver), (5, 4));
    // Through the other seats' turns and into the next upkeep, while the next trigger waits.
    let second = first + u32::try_from(seats).unwrap();
    driver.idle_until(second - 1, Step::Main1)?;
    assert_eq!(size(&driver), (5, 4));
    target_in_upkeep(&mut driver, second, "Wall")?;
    assert_eq!(driver.state().stack().len(), 1);
    assert_eq!(
        size(&driver),
        (5, 4),
        "it lasts until the end of the upkeep"
    );
    resolve(&mut driver)?;
    assert_eq!(size(&driver), (1, 6), "the newer size wins");
    driver.idle_until(second, Step::Draw)?;
    assert_eq!(size(&driver), (1, 6));
    let setup = table(seats).setup()?;
    let log = driver.engine().events().to_vec();
    assert_eq!(
        replay(&setup, &log).unwrap().state_hash(),
        driver.state().state_hash()
    );
    let resumed = Engine::resume(setup, log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    Ok(())
}
