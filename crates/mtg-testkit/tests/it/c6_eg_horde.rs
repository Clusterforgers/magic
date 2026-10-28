//! Goal: Unbreathing Horde enters with a +1/+1 counter for each other Zombie its controller controls and each Zombie card in their graveyard, damage to it is prevented and takes one counter, and with no Zombie around it enters with none and dies as a 0/0, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, on_battlefield};

const C6_EG_HORDE: &str = "Unbreathing Horde";

fn c6_eg_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn c6_eg_counters(driver: &Driver) -> Option<u32> {
    let id = on_battlefield(driver.state(), C6_EG_HORDE)?;
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map(|held| held.counters_of(&CounterKind::PlusOnePlusOne))
}

#[rstest]
fn c6_eg_the_horde_counts_zombies_and_pays_a_counter_for_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] on_field: usize,
    #[values(0, 2)] in_graveyard: usize,
) -> Result<(), ScriptError> {
    let zombie = || card("Dregscape Zombie").unwrap();
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(C6_EG_HORDE).unwrap(), burn("Bolt", 3)])
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(swamp()))
                .chain([ready(mountain())])
                .chain((0..on_field).map(|_| ready(zombie()))),
        )
        .graveyard(0, (0..in_graveyard).map(|_| zombie()));
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping(C6_EG_HORDE, &["Swamp", "Swamp", "Swamp"])?;
    c6_eg_settle(&mut driver)?;
    let entered = u32::try_from(on_field + in_graveyard).unwrap();
    assert_eq!(c6_eg_counters(&driver), (entered > 0).then_some(entered));
    if entered == 0 {
        assert!(
            names(driver.state(), Zone::Graveyard(PlayerId(0))).contains(&C6_EG_HORDE.to_string())
        );
        return Ok(());
    }
    let target = driver.target_named(C6_EG_HORDE)?;
    driver.cast_at("Bolt", &[target])?;
    c6_eg_settle(&mut driver)?;
    assert_eq!(c6_eg_counters(&driver), (entered > 1).then(|| entered - 1));
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::DamageDealt { target, .. } if format!("{target:?}").contains("Object"))),
        "no damage is dealt to the Horde"
    );
    Ok(())
}
