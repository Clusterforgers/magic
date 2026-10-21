//! Goal: shared setup for the combat test files (this is not a test module itself).

use mtg_engine::PresetPermanent;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, damaged, ready};

use super::names;

/// A creature of a combat table: name, power, toughness and damage already marked.
pub type Fighter = (&'static str, i32, i32, u32);

/// The fighters as permanents that start untapped and ready to act.
pub fn creatures(list: &[Fighter]) -> Vec<PresetPermanent> {
    list.iter()
        .map(|&(name, power, toughness, marked)| {
            damaged(free_creature(name, power, toughness), marked)
        })
        .collect()
}

/// Two players, P0 about to declare attackers with `A`, P1 holding `B`.
pub fn duel(attacker: (i32, i32), blocker: (i32, i32)) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(free_creature("A", attacker.0, attacker.1))])
            .battlefield(1, [ready(free_creature("B", blocker.0, blocker.1))]),
    )
}

/// Passes priority until the blockers decision (after the declare-attackers priority round).
pub fn to_blockers(driver: &mut Driver, defenders: usize) -> Result<(), ScriptError> {
    for _ in 0..driver.state().players().len() {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.pass()?;
    }
    let _ = defenders;
    Ok(())
}

/// An id that exists nowhere, so a failed lookup turns into an illegal answer rather than a panic.
pub fn missing_if_none(id: Option<mtg_core::ids::ObjectId>) -> mtg_core::ids::ObjectId {
    id.unwrap_or(mtg_core::ids::ObjectId(u64::MAX))
}

pub fn survivors(driver: &Driver) -> Vec<String> {
    let mut alive = names(driver.state(), Zone::Battlefield);
    alive.sort();
    alive
}
