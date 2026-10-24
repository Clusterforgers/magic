//! Goal: a copy of a spell whose own effect exiles it (Twincast on Teferi's Protection) resolves whole and ceases to exist, the original resolves after it and is exiled too, and the game goes on with priority for the active player and replays from its log, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{island, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

#[rstest]
fn a_copy_that_exiles_itself_ends_its_resolution(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Teferi's Protection", "Twincast"])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(plains()),
                ready(plains()),
                ready(island()),
                ready(island()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping("Teferi's Protection", &["Plains", "Plains", "Plains"])?;
    let on_stack = *driver.state().stack().last().unwrap();
    let spell = Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.begin_cast("Twincast")?;
    driver.choose_targets(&[spell])?;
    driver.pay_tapping(&["Island", "Island"], 0)?;
    let copies = |driver: &Driver| {
        driver
            .engine()
            .events()
            .iter()
            .filter(|event| matches!(event, Event::SpellCopied(_)))
            .count()
    };
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(copies(&driver), 1, "Twincast copied the spell");
    assert!(driver.state().stack().is_empty());
    let exiled: Vec<String> = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    assert_eq!(exiled, ["Teferi's Protection"], "the copy ceased to exist");
    assert_eq!(
        driver.decision()?.player,
        pid(0),
        "the active player has priority"
    );
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
