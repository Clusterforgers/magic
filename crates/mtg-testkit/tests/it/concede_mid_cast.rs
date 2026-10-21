//! Goal: conceding while a spell is half-cast abandons that cast cleanly, whether the conceder is the caster or someone else.

use crate::common;

use common::{names, pid};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event, GameResult};
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

/// Seat 0 has chosen to cast Grizzly Bears and is about to pay.
fn mid_cast(players: usize) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(players)
            .hand(0, [bear()])
            .battlefield(0, [ready(forest()), ready(forest())]),
    )?;
    driver.begin_cast("Grizzly Bears")?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    Ok(driver)
}

#[test]
fn the_caster_conceding_drops_the_cast_and_the_game_moves_on() -> Result<(), ScriptError> {
    let mut driver = mid_cast(3)?;
    driver.concede(pid(0))?;
    let decision = driver.decision()?;
    assert_ne!(decision.player, pid(0));
    assert!(!matches!(decision.kind, DecisionKind::PayMana(_)));
    assert!(driver.state().objects().all(|o| o.owner() != pid(0)));
    assert_eq!(
        driver.state().turn().active(),
        pid(1),
        "seat 0's turn is over"
    );
    Ok(())
}

#[test]
fn another_player_conceding_sends_the_caster_back_to_priority() -> Result<(), ScriptError> {
    let mut driver = mid_cast(3)?;
    driver.concede(pid(2))?;
    let decision = driver.decision()?;
    assert_eq!(decision.player, pid(0));
    assert!(matches!(decision.kind, DecisionKind::Priority(_)));
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Grizzly Bears"]);
    assert!(
        driver.state().zone(Zone::Stack).is_empty(),
        "nothing was cast"
    );

    let stale = driver.answer(Answer::PayMana(PayChoice::Auto));
    assert!(matches!(
        stale,
        Err(ScriptError::Engine(EngineError::Illegal(_)))
    ));
    driver.cast("Grizzly Bears")?;
    assert_eq!(names(driver.state(), Zone::Stack), ["Grizzly Bears"]);
    Ok(())
}

#[test]
fn the_only_opponent_conceding_ends_the_game_even_mid_cast() -> Result<(), ScriptError> {
    let mut driver = mid_cast(2)?;
    let events = driver.concede(pid(1))?;
    assert!(matches!(
        events.last(),
        Some(Event::GameEnded {
            result: GameResult::Winner(winner)
        }) if *winner == pid(0)
    ));
    assert!(driver.engine().decision().is_none());
    assert_eq!(
        driver.answer(Answer::PayMana(PayChoice::Auto)),
        Err(ScriptError::Engine(EngineError::NoDecision))
    );
    Ok(())
}
