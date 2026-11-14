//! Goal: with Abandoned Sarcophagus, a card with cycling that is cycled goes to the graveyard, may then be cast from
//! there, and when it would go to the graveyard again without being cycled it is exiled instead; at two to four seats,
//! and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Passes, answering every question passively, until the stack is empty.
fn r6105_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if matches!(driver.decision()?.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("settling".to_owned()))
}

/// Whether a card called `name` lies in the zone.
fn r6105_in(driver: &Driver, zone: Zone, name: &str) -> bool {
    driver
        .state()
        .zone(zone)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .any(|object| object.card().name == name)
}

#[rstest]
fn r6105_cycled_then_cast_then_exiled(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .library(0, vec![bear(); 10])
        .hand(0, [card("Hampering Snare").unwrap()])
        .battlefield(
            0,
            [ready(card("Abandoned Sarcophagus").unwrap()), ready(bear())]
                .into_iter()
                .chain((0..5).map(|_| ready(island()))),
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Hampering Snare")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r6105_settle(&mut driver)?;
    assert!(r6105_in(&driver, Zone::Graveyard(pid(0)), "Hampering Snare"), "a cycled card is not exiled");
    driver.cast("Hampering Snare")?;
    r6105_settle(&mut driver)?;
    assert!(!r6105_in(&driver, Zone::Graveyard(pid(0)), "Hampering Snare"));
    assert!(r6105_in(&driver, Zone::Exile, "Hampering Snare"), "cast from the graveyard, then exiled");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
