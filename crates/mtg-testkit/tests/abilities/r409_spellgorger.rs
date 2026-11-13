//! Goal: a predefined token with a mana cost plays as printed at two to four seats: chapter III of Ral and the Implicit
//! Maze creates a Spellgorger Weird, a red 2/2 Weird whose mana value is 3 from its mana cost {2}{R}, which gets a +1/+1
//! counter each time its controller casts a noncreature spell and none for a creature spell.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, instant};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Priority passes until the stack is empty.
fn r409_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
fn r409_ral_makes_a_spellgorger_weird_that_grows_on_noncreature_spells(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ral = with_counters(
        ready(card("Ral and the Implicit Maze").unwrap()),
        CounterKind::lore(),
        2,
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, [ral])
        .hand(
            0,
            [
                instant("Spark", 0, &[]),
                instant("Flare", 0, &[]),
                free_creature("Squire", 1, 1),
            ],
        )
        .at(1, 0, Step::Upkeep);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    r409_settle(&mut driver)?;
    let weird = driver.id_of("Spellgorger Weird")?;
    let object = driver.state().object(weird).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    let cost = object.card().mana_cost.clone().unwrap();
    assert_eq!(cost.mana_value(), 3);
    let seen = characteristics(driver.state(), weird).unwrap();
    assert!(seen.colors.contains(Color::Red));
    assert_eq!((seen.power, seen.toughness), (Some(2), Some(2)));
    let counters = |driver: &Driver| {
        driver
            .state()
            .object(weird)
            .and_then(|object| object.permanent())
            .map_or(0, |permanent| {
                permanent.counters_of(&CounterKind::PlusOnePlusOne)
            })
    };
    assert_eq!(counters(&driver), 0);
    driver.cast("Spark")?;
    r409_settle(&mut driver)?;
    assert_eq!(counters(&driver), 1);
    driver.cast("Squire")?;
    r409_settle(&mut driver)?;
    assert_eq!(counters(&driver), 1, "a creature spell feeds it nothing");
    driver.cast("Flare")?;
    r409_settle(&mut driver)?;
    assert_eq!(counters(&driver), 2);
    Ok(())
}
