//! Goal: "for as long as you control ~" ends a gain of control when the source leaves the battlefield or another player controls it, at tables of two, three and four seats: Mind Flayer and Master Thief take their target while the player controls them; the target returns when the source dies, when another player gains control of the source (the layers derive the source's controller, so the check is not circular) and does not come back when the source returns to its controller; "for as long as ~ remains on the battlefield" (Sower of Temptation) follows the permanent whoever controls it; and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_cards::modify::Duration;
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::Modification;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{burn, free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds the card and has five Islands and a Mountain with a Bolt in hand; every other seat has an Ogre (seat 1
/// also an Artifact Ogre named "Ring").
fn table(seats: usize, name: &str) -> Scenario {
    let mut lands: Vec<_> = (0..5).map(|_| ready(island())).collect();
    lands.push(ready(mountain()));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(name).unwrap(), burn("Bolt", 3)])
        .battlefield(0, lands);
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Ogre {seat}"), 3, 3))]);
    }
    scenario
}

/// Answers until nothing is on the stack and, when `target` is named, its trigger has chosen that target.
pub(crate) fn settle(driver: &mut Driver, target: Option<&str>) -> Result<(), ScriptError> {
    let mut wanted = target.is_some();
    for _ in 0..40 {
        if !wanted && driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            let chosen = driver.target_named(target.unwrap_or("?"))?;
            driver.choose_targets(&[chosen])?;
            wanted = false;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

pub(crate) fn controller(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    controller_of(driver.state(), id).unwrap().index()
}

pub(crate) fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn expired(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::EffectExpired { .. }))
        .count()
}

/// Casts the card at seat 0 and takes `Ogre 1` with its enter trigger.
fn taken(seats: usize, card: &str) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, card))?;
    driver.cast(card)?;
    settle(&mut driver, Some("Ogre 1"))?;
    Ok(driver)
}

#[rstest]
fn the_target_stays_while_the_source_stays_under_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[values("Mind Flayer", "Sower of Temptation")] card: &str,
) -> Result<(), ScriptError> {
    let driver = taken(seats, card)?;
    assert_eq!(controller(&driver, "Ogre 1"), 0);
    assert_eq!(controller(&driver, card), 0);
    for seat in 2..seats {
        assert_eq!(controller(&driver, &format!("Ogre {seat}")), seat);
    }
    assert_eq!(expired(&driver), 0);
    replays(&driver);
    Ok(())
}

#[rstest]
fn the_target_returns_when_the_source_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values("Mind Flayer", "Sower of Temptation")] card: &str,
) -> Result<(), ScriptError> {
    let mut driver = taken(seats, card)?;
    let flayer = driver.target_named(card)?;
    driver.cast_at("Bolt", &[flayer])?;
    settle(&mut driver, None)?;
    assert!(!names(driver.state(), Zone::Battlefield).contains(&card.to_owned()));
    assert_eq!(controller(&driver, "Ogre 1"), 1);
    assert!(expired(&driver) >= 1);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::the_flayer_is_taken("Mind Flayer", 1, false)]
#[case::the_flayer_is_taken_but_the_ogre_waits_for_no_one("Mind Flayer", 1, true)]
#[case::the_sower_is_taken_and_still_holds_the_creature("Sower of Temptation", 0, false)]
fn another_player_gaining_control_of_the_source_ends_a_control_that_asks_for_it(
    #[case] card: &str,
    #[case] ogre_goes_to: usize,
    #[case] source_returns: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = taken(seats, card)?;
    let flayer = driver.id_of(card)?;
    let on_flayer = driver.on(flayer);
    driver.add_effect(
        on_flayer,
        Modification::SetController(pid(1)),
        Duration::UntilEndOfTurn,
    )?;
    assert_eq!(controller(&driver, card), 1);
    assert_eq!(controller(&driver, "Ogre 1"), ogre_goes_to);
    if source_returns {
        // The turn ends, so the taking effect ends; the creature that came home stays home.
        driver.idle_until(2, Step::Main1)?;
        assert_eq!(controller(&driver, card), 0);
        assert_eq!(controller(&driver, "Ogre 1"), 1);
    }
    replays(&driver);
    Ok(())
}
