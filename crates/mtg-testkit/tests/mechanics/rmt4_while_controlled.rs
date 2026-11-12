//! Goal: "You may play that card for as long as you control ~" (Lightning, Security Sergeant) lets the controller play the exiled card while they control the source, at tables of two, three and four seats; the permission ends when the source dies or another player gains control of it, and does not come back when the source returns; every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::modify::Duration;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::Modification;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::c4_control::{replays, settle};
use crate::common::{name_of, pid};

const LIGHTNING: &str = "Lightning, Security Sergeant";

/// Seat 0 has Lightning, a Mountain and a Bolt in hand over a library of Scouts; the other seats have nothing.
fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(LIGHTNING).unwrap()), ready(mountain())])
        .hand(0, [burn("Bolt", 3)])
        .library(0, (0..5).map(|_| free_creature("Scout", 1, 1)))
}

/// The Scout in exile, if any.
fn exiled_scout(driver: &Driver) -> Option<ObjectId> {
    driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Scout")
}

/// Whether seat 0 may cast the exiled Scout at its priority.
fn castable(driver: &Driver) -> bool {
    let Some(scout) = exiled_scout(driver) else {
        return false;
    };
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.contains(&scout),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// Lightning attacks seat 1 and deals its combat damage; the trigger exiles the top card; seat 0 is in its second main phase.
fn struck(seats: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(LIGHTNING, 1)])?;
    driver.pass_until(Step::Main2)?;
    settle(&mut driver, None)?;
    assert!(exiled_scout(&driver).is_some(), "the trigger exiled a card");
    Ok(driver)
}

#[rstest]
fn the_exiled_card_is_playable_while_you_control_the_source(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = struck(seats)?;
    assert!(castable(&driver));
    driver.cast("Scout")?;
    settle(&mut driver, None)?;
    assert!(driver.id_of("Scout").is_ok());
    replays(&driver);
    Ok(())
}

#[rstest]
fn the_permission_ends_when_the_source_dies(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = struck(seats)?;
    let lightning = driver.target_named(LIGHTNING)?;
    driver.cast_at("Bolt", &[lightning])?;
    settle(&mut driver, None)?;
    assert!(driver.id_of(LIGHTNING).is_err());
    assert!(!castable(&driver));
    replays(&driver);
    Ok(())
}

#[rstest]
fn the_permission_ends_when_another_player_controls_the_source_and_stays_ended(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = struck(seats)?;
    let lightning = driver.id_of(LIGHTNING)?;
    let on = driver.on(lightning);
    driver.add_effect(
        on,
        Modification::SetController(pid(1)),
        Duration::UntilEndOfTurn,
    )?;
    assert!(!castable(&driver));
    // The taking ends with the turn; on seat 0's next turn it controls Lightning again, and the card stays exiled.
    driver.idle_until(u32::try_from(seats).unwrap() + 1, Step::Main1)?;
    assert_eq!(crate::c4_control::controller(&driver, LIGHTNING), 0);
    assert!(exiled_scout(&driver).is_some());
    assert!(!castable(&driver));
    replays(&driver);
    Ok(())
}
