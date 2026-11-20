//! Goal: counters divided among targets come off again later, one for each counter put on that creature this way (CR
//! 603.7c), at tables of two to four seats: Bounty of the Hunt's +1/+1 counters at the next cleanup step and Shambling
//! Swarm's -1/-1 counters at the next end step, each target losing only its own share, counters it had before staying,
//! and the log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::{DivideChoice, Target};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, GameSetup, replay};
use mtg_testkit::cards::{burn, forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(kind))
}

/// Seat 0 holds `hand` with five Forests and a Mountain and controls Bear (2/2, one +1/+1 counter) and `own`; the last
/// seat controls Ogre (4/4) and Giant (5/5).
fn r110_table(seats: usize, own: Vec<mtg_engine::PresetPermanent>) -> Scenario {
    let mut mine = vec![
        ready(forest()),
        ready(forest()),
        ready(forest()),
        ready(forest()),
        ready(forest()),
        ready(mountain()),
        with_counters(
            ready(free_creature("Bear", 2, 2)),
            CounterKind::PlusOnePlusOne,
            1,
        ),
    ];
    mine.extend(own);
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(
            seats - 1,
            [
                ready(free_creature("Ogre", 4, 4)),
                ready(free_creature("Giant", 5, 5)),
            ],
        )
}

/// Answers every decision passively until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn division(driver: &Driver, shares: &[(&str, u32)]) -> (Vec<Target>, Answer) {
    let chosen: Vec<Target> = shares
        .iter()
        .map(|(name, _)| driver.target_named(name).unwrap())
        .collect();
    let split = chosen
        .iter()
        .copied()
        .zip(shares.iter().map(|(_, share)| *share))
        .collect();
    (chosen, Answer::DivideAmount(DivideChoice::Division(split)))
}

#[rstest]
fn r110_bounty_counters_come_off_at_cleanup(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || r110_table(seats, Vec::new()).hand(0, [card("Bounty of the Hunt").unwrap()]);
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_cast("Bounty of the Hunt")?;
    while !matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        driver.answer_passively()?;
    }
    let (chosen, divide) = division(&driver, &[("Bear", 2), ("Ogre", 1)]);
    driver.choose_targets(&chosen)?;
    driver.answer(divide)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    let plus = CounterKind::PlusOnePlusOne;
    assert_eq!(counters(&driver, "Bear", &plus), 3);
    assert_eq!(counters(&driver, "Ogre", &plus), 1);
    driver.idle_until(2, Step::End)?;
    assert_eq!(
        counters(&driver, "Bear", &plus),
        3,
        "still on in the end step"
    );
    driver.idle_until(3, Step::Upkeep)?;
    assert_eq!(
        counters(&driver, "Bear", &plus),
        1,
        "the counter it had stays"
    );
    assert_eq!(counters(&driver, "Ogre", &plus), 0);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn r110_swarm_counters_come_off_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] split: bool,
) -> Result<(), ScriptError> {
    let table = || {
        r110_table(seats, vec![ready(card("Shambling Swarm").unwrap())]).hand(0, [burn("Zap", 3)])
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    let swarm = driver.target_named("Shambling Swarm")?;
    driver.cast_at("Zap", &[swarm])?;
    for _ in 0..50 {
        match driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let shares: &[(&str, u32)] = if split {
        &[("Ogre", 2), ("Giant", 1)]
    } else {
        &[("Giant", 3)]
    };
    let (chosen, divide) = division(&driver, shares);
    driver.choose_targets(&chosen)?;
    if split {
        driver.answer(divide)?;
    }
    settle(&mut driver)?;
    let minus = CounterKind::MinusOneMinusOne;
    let (ogre, giant) = if split { (2, 1) } else { (0, 3) };
    assert_eq!(counters(&driver, "Ogre", &minus), ogre);
    assert_eq!(counters(&driver, "Giant", &minus), giant);
    driver.idle_until(2, Step::End)?;
    settle(&mut driver)?;
    assert_eq!(counters(&driver, "Ogre", &minus), 0);
    assert_eq!(counters(&driver, "Giant", &minus), 0);
    replays(&driver, &setup);
    Ok(())
}
