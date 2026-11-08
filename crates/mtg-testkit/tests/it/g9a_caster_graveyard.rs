//! Goal: "Whenever an opponent casts a spell with the same name as a card in their graveyard" reads the caster's
//! graveyard (the spell's controller as `Candidate`), not that of the trigger's controller, at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "Whenever an opponent casts an instant spell with the same name as a card in their graveyard, that player loses
/// 10 life", as the translator writes Dragonlord Kolaghan's trigger.
fn warden() -> Arc<CardDefinition> {
    let rules = r#"{"Triggered":{"when":{"SpellCast":{"by":"Opponents","what":[{"Type":"Instant"},{"SharesName":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"ControllerOf":"Candidate"}}]}}]}},"effects":[{"LoseLife":{"who":"Triggering","amount":10}}]}}"#;
    Arc::new(
        CardDefinition::new("Warden")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

/// Passes priority until `seat` holds it.
fn until_held_by(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(seat)
        || !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
    {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::bolt_in_the_casters_graveyard(1, 10)]
#[case::bolt_in_the_wardens_graveyard(0, 0)]
fn the_casters_graveyard_is_read(
    #[case] holder: usize,
    #[case] lost: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(warden())])
        .graveyard_named(holder, ["Lightning Bolt"])
        .hand_named(1, ["Lightning Bolt"])
        .battlefield(1, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    until_held_by(&mut driver, 1)?;
    let before = driver.state().player(pid(1)).unwrap().life();
    let target = driver.target_player(0);
    driver.cast_at("Lightning Bolt", &[target])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    let after = driver.state().player(pid(1)).unwrap().life();
    assert_eq!(before - after, lost);
    Ok(())
}
