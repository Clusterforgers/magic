//! Goal: selectors that compare with another object play as printed at tables of two, three and four seats: Maelstrom
//! Pulse destroys every permanent with the target's name and no other, Cleansing Beam hurts only the creatures that
//! share a color with its target, Martyr's Bond makes each opponent sacrifice a permanent that shares a card type with
//! the permanent that left (read as it last existed), and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, creature, forest, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_engine::PresetPermanent;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;

/// Answers passively until the stack and the waiting triggers are empty.
fn w6d_settle(driver: &mut Driver) -> Result<(), ScriptError> {
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

/// The creatures on the battlefield that `seat` controls.
fn w6d_creatures(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.controller() == PlayerId::from_index(seat))
        .filter(|object| object.card().types.contains(&CardType::Creature))
        .count()
}

/// Seat 0 holds `spell` and controls two of each basic land it needs; every seat controls `theirs`.
fn w6d_table(seats: usize, spell: &str, mine: Vec<PresetPermanent>) -> Scenario {
    let mut lands = mine;
    for _ in 0..2 {
        lands.extend([ready(swamp()), ready(forest()), ready(mountain())]);
    }
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(spell).unwrap()])
        .library(0, (0..5).map(|_| bear()))
        .battlefield(0, lands)
}

#[rstest]
fn w6d_maelstrom_pulse_destroys_every_permanent_with_the_name(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let giant = creature("Hill Giant", 3, &[Color::Red], 3, 3);
    let mut scenario = w6d_table(seats, "Maelstrom Pulse", vec![ready(giant.clone())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear()), ready(giant.clone())]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Hill Giant")?;
    driver.cast_at("Maelstrom Pulse", &[target])?;
    w6d_settle(&mut driver)?;
    assert_eq!(w6d_creatures(&driver, 0), 0, "every Hill Giant is destroyed");
    for seat in 1..seats {
        assert_eq!(w6d_creatures(&driver, seat), 1, "the bear has another name");
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w6d_cleansing_beam_hurts_the_creatures_that_share_a_color(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let red = creature("Red Cub", 1, &[Color::Red], 2, 2);
    let green = creature("Green Cub", 1, &[Color::Green], 2, 2);
    let gold = creature("Gold Cub", 0, &[Color::Red, Color::Green], 2, 2);
    let mut scenario = w6d_table(seats, "Cleansing Beam", Vec::new());
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [ready(red.clone()), ready(green.clone()), ready(gold.clone())],
        );
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Red Cub")?;
    driver.cast_at("Cleansing Beam", &[target])?;
    w6d_settle(&mut driver)?;
    for seat in 1..seats {
        assert_eq!(w6d_creatures(&driver, seat), 1, "only the green creature stays");
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w6d_martyrs_bond_takes_a_permanent_of_a_shared_type(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let wolf = creature("Lone Wolf", 1, &[Color::Green], 2, 2);
    let bond = ready(card("Martyr's Bond").unwrap());
    let mut scenario = w6d_table(seats, "Maelstrom Pulse", vec![ready(wolf), bond]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear()), ready(swamp())]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Lone Wolf")?;
    driver.cast_at("Maelstrom Pulse", &[target])?;
    w6d_settle(&mut driver)?;
    for seat in 1..seats {
        assert_eq!(w6d_creatures(&driver, seat), 0, "a creature is sacrificed");
        let lands = driver.state().zone(Zone::Battlefield).iter().filter(|id| {
            driver.state().object(**id).is_some_and(|object| {
                object.controller() == PlayerId::from_index(seat)
                    && object.card().types.contains(&CardType::Land)
            })
        });
        assert_eq!(lands.count(), 1, "the land shares no type with the creature");
    }
    replays(&driver, &setup);
    Ok(())
}
