//! Goal: corpus cards of the r28-1 pack play as their rules say at two and four seats: Ersatz Gnomes makes a spell colorless until it leaves the stack and a permanent colorless until end of turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::Answer;
use mtg_engine::choose::TargetChoice;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

#[rstest]
fn ersatz_gnomes_makes_a_spell_colorless(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Grizzly Bears"])
            .battlefield_named(0, ["Ersatz Gnomes", "Forest", "Forest"]),
    )?;
    driver.begin_cast("Grizzly Bears")?;
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        let answer = match driver.decision()?.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(mtg_engine::decision::PayChoice::Auto),
            _ => Driver::passive_answer(driver.decision()?),
        };
        driver.answer(answer)?;
    }
    let bears = *driver.state().stack().last().unwrap();
    assert!(
        !mtg_engine::characteristics(driver.state(), bears)
            .unwrap()
            .colors
            .is_empty()
    );
    let first = driver.offered_abilities("Ersatz Gnomes")?[0];
    driver.begin_ability("Ersatz Gnomes", first)?;
    let spell = driver.state().object(bears).unwrap().reference();
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
        mtg_engine::choose::Target::Object(spell),
    ])))?;
    // The Gnomes' ability resolves; the spell is still on the stack.
    driver.pass()?;
    for _ in 1..seats {
        driver.pass()?;
    }
    assert_eq!(driver.state().stack().last(), Some(&bears));
    assert!(
        mtg_engine::characteristics(driver.state(), bears)
            .unwrap()
            .colors
            .is_empty()
    );
    Ok(())
}
