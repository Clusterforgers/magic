//! Goal: an amount counted over what the resolving spell destroyed ("for each creature destroyed this way", "the
//! number of creatures you controlled that were destroyed this way") plays as printed at tables of two, three and four
//! seats: Fumigate, Kaya's Wrath and Decree of Pain count every creature they destroyed, or only their caster's, never
//! an indestructible one that stayed, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, keyworded_creature, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

const LIFE: i32 = 20;

/// Answers passively until the stack and the waiting triggers are empty.
fn w2c_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 holds `spell` and controls four Plains, four Swamps, two Grizzly Bears and an indestructible creature; every
/// other seat controls a Grizzly Bears. Seat 0's library has ten cards.
fn w2c_table(seats: usize, spell: &str) -> Scenario {
    let wall = keyworded_creature("Stone Wall", 0, &[], 0, 4, &[Keyword::Indestructible]);
    let mut mine = vec![ready(bear()), ready(bear()), ready(wall)];
    mine.extend((0..4).map(|_| ready(plains())));
    mine.extend((0..4).map(|_| ready(swamp())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(spell).unwrap()])
        .library(0, (0..10).map(|_| bear()))
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

/// Casts the spell seat 0 holds and lets it resolve.
fn w2c_cast(seats: usize, spell: &str) -> Result<Driver, ScriptError> {
    let scenario = w2c_table(seats, spell);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(spell)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    w2c_settle(&mut driver)?;
    assert_eq!(
        driver.state().zone(Zone::Battlefield).len(),
        9,
        "the lands and the indestructible creature stay"
    );
    replays(&driver, &setup);
    Ok(driver)
}

fn w2c_life(driver: &Driver) -> i32 {
    driver.state().player(PlayerId(0)).unwrap().life()
}

#[rstest]
fn w2c_fumigate_gains_a_life_for_each_creature_destroyed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = w2c_cast(seats, "Fumigate")?;
    let destroyed = i32::try_from(seats).unwrap() + 1;
    assert_eq!(w2c_life(&driver), LIFE + destroyed);
    Ok(())
}

#[rstest]
fn w2c_kayas_wrath_counts_only_the_casters_creatures(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = w2c_cast(seats, "Kaya's Wrath")?;
    assert_eq!(w2c_life(&driver), LIFE + 2);
    Ok(())
}

#[rstest]
fn w2c_decree_of_pain_draws_for_each_creature_destroyed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = w2c_cast(seats, "Decree of Pain")?;
    assert_eq!(
        driver.state().zone(Zone::Hand(PlayerId(0))).len(),
        seats + 1
    );
    Ok(())
}
