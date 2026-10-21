//! Goal: restoring a checkpoint returns an engine to exactly where it was, whatever step followed (answer, concession, half-finished cast), so a step that cannot be persisted can be undone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Engine, EngineError, Event, replay};
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::{RandomBot, Scenario, fuzz_setup, ready};
use rstest::rstest;

/// What identifies an engine's position: state, log length and pending question.
fn position(engine: &Engine) -> (u64, usize, Option<mtg_engine::Decision>) {
    (
        engine.state_hash(),
        engine.events().len(),
        engine.decision().cloned(),
    )
}

fn started(players: usize, seed: u64, answers: usize) -> (Engine, Vec<RandomBot>) {
    let mut engine = Engine::new(fuzz_setup(players, seed)).unwrap();
    let mut bots: Vec<RandomBot> = (0u64..)
        .take(players)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    for _ in 0..answers {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        engine
            .apply(bots[decision.player.index()].choose(&decision))
            .unwrap();
    }
    (engine, bots)
}

#[rstest]
fn restoring_undoes_a_step_and_redoing_it_gives_the_same_result(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 1, 7, 40, 160)] answers: usize,
    #[values(1, 2)] seed: u64,
) {
    let (mut engine, mut bots) = started(players, seed, answers);
    for _ in 0..25 {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let answer = bots[decision.player.index()].choose(&decision);
        let before = position(&engine);
        let checkpoint = engine.checkpoint();

        let first = engine.apply(answer.clone()).unwrap();
        let after = position(&engine);
        engine.restore(checkpoint).unwrap();
        assert_eq!(position(&engine), before);
        let replayed = replay(engine.setup(), engine.events()).unwrap();
        assert_eq!(replayed.state_hash(), engine.state_hash());

        let second = engine.apply(answer).unwrap();
        assert_eq!(first, second);
        assert_eq!(position(&engine), after);
    }
}

#[rstest]
fn restoring_undoes_a_concession(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 30, 120)] answers: usize,
) {
    let (mut engine, _) = started(players, 3, answers);
    let before = position(&engine);
    let checkpoint = engine.checkpoint();
    let events = engine.concede(pid(players - 1)).unwrap();
    assert!(matches!(events.first(), Some(Event::PlayerLost { .. })));
    engine.restore(checkpoint).unwrap();
    assert_eq!(position(&engine), before);
    assert!(engine.state().is_alive(pid(players - 1)));
}

#[test]
fn a_checkpoint_taken_mid_cast_brings_the_payment_decision_back() {
    let mut engine = Scenario::new(2)
        .hand(0, [bear()])
        .battlefield(0, [ready(forest()), ready(forest())])
        .build()
        .unwrap();
    let Some(DecisionKind::Priority(options)) = engine.decision().map(|d| d.kind.clone()) else {
        panic!("expected priority");
    };
    engine
        .apply(Answer::Priority(PriorityAction::CastSpell(
            options.castable[0],
        )))
        .unwrap();
    assert!(matches!(
        engine.decision().map(|d| &d.kind),
        Some(DecisionKind::PayMana(_))
    ));
    let paying = position(&engine);
    let checkpoint = engine.checkpoint();

    engine.apply(Answer::PayMana(PayChoice::Cancel)).unwrap();
    assert!(matches!(
        engine.decision().map(|d| &d.kind),
        Some(DecisionKind::Priority(_))
    ));
    engine.restore(checkpoint).unwrap();
    assert_eq!(position(&engine), paying);

    let events = engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
}

#[test]
fn a_checkpoint_beyond_the_log_is_rejected_and_changes_nothing() {
    let (mut engine, mut bots) = started(2, 5, 10);
    let early = engine.checkpoint();
    let decision = engine.decision().cloned().unwrap();
    engine
        .apply(bots[decision.player.index()].choose(&decision))
        .unwrap();
    let late = engine.checkpoint();
    engine.restore(early).unwrap();

    let before = position(&engine);
    assert!(matches!(
        engine.restore(late),
        Err(EngineError::InvalidEvent(_))
    ));
    assert_eq!(position(&engine), before);
}
