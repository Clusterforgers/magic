//! Goal: a creature mentors a creature only when its mentor ability resolves and puts the counter (CR 702.134a), and
//! Aegis of the Legion then puts a shield counter on the creature mentored, at two to four seats; a mentor of its own
//! on an unequipped creature gives no shield.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The counters of a kind on the only permanent of this name.
fn r6401_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .unwrap()
        .counters()
        .filter(|(held, _)| *held == kind)
        .map(|(_, n)| n)
        .sum()
}

/// Answers passively until seat 0 has priority with an empty stack.
fn r6401_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && decision.player == pid(0)
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("settle".into()))
}

#[rstest]
#[case::the_equipped_creature_mentors(true, 1, 1)]
#[case::an_unequipped_aegis_gives_nothing(false, 1, 0)]
fn r6401_aegis_shields_the_creature_mentored(
    #[values(2, 3, 4)] seats: usize,
    #[case] equipped: bool,
    #[case] plus: u32,
    #[case] shields: u32,
) -> Result<(), ScriptError> {
    let aegis = ready(card("Aegis of the Legion").unwrap());
    let aegis = if equipped {
        attached_to(aegis, 0, 0)
    } else {
        aegis
    };
    // An unattached Aegis gives no mentor, so a creature with mentor of its own attacks instead.
    let big = if equipped {
        free_creature("Big", 3, 3)
    } else {
        mtg_testkit::cards::keyworded("Big", 3, 3, &[mtg_cards::keyword::Keyword::Mentor])
    };
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(big), aegis, ready(free_creature("Small", 1, 1))]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Watcher", 0, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Big", 1), ("Small", 1)])?;
    r6401_settle(&mut driver)?;
    assert_eq!(
        r6401_counters(&driver, "Small", &CounterKind::PlusOnePlusOne),
        plus
    );
    let shield = CounterKind::Other("shield".to_owned());
    assert_eq!(r6401_counters(&driver, "Small", &shield), shields);
    assert_eq!(r6401_counters(&driver, "Big", &shield), 0);
    Ok(())
}
