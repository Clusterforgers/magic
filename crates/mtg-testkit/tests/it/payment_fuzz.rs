//! Goal: in whole bot games, a cancelled cast changes nothing and leaves nothing in the log, and a completed partial payment taps what was chosen plus the rest.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::{Decision, Engine, Event};
use mtg_testkit::{RandomBot, fuzz_setup};
use rstest::rstest;

/// Games per table size; the full campaign is `fuzz_games.rs`, this one only needs enough casts to cover each choice.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(120, |n: u64| n.min(120))
}

#[derive(Debug, Default)]
struct Tally {
    cancelled: u32,
    completed: u32,
    automatic: u32,
    manual: u32,
}

#[rstest]
#[case(2)]
#[case(4)]
fn cancel_and_complete_hold_in_every_position(#[case] players: usize) {
    let mut tally = Tally::default();
    for seed in 0..games() {
        let setup = fuzz_setup(players, seed);
        let mut engine = Engine::new(setup.clone()).expect("a fuzz setup is valid");
        let mut bots: Vec<RandomBot> = (0u64..)
            .take(players)
            .map(|seat| RandomBot::new(seed, seat).with_partial_payments())
            .collect();
        let mut previous: Option<Decision> = None;
        while let Some(decision) = engine.decision().cloned() {
            let answer = bots[decision.player.index()].choose(&decision);
            let (hash, log) = (engine.state_hash(), engine.events().len());
            let events = engine
                .apply(answer.clone())
                .unwrap_or_else(|e| panic!("seed {seed}: {e} for {answer:?}"));
            if let (DecisionKind::PayMana(_), Answer::PayMana(choice)) = (&decision.kind, &answer) {
                match choice {
                    PayChoice::Cancel => {
                        tally.cancelled += 1;
                        cancel_leaves_no_trace(
                            &setup,
                            &engine,
                            &events,
                            (hash, log),
                            previous.as_ref(),
                        );
                    }
                    PayChoice::Complete(partial) => {
                        tally.completed += 1;
                        for tap in &partial.taps {
                            assert!(
                                events.iter().any(|e| matches!(e, Event::PermanentTapped { object } if *object == tap.object)),
                                "seed {seed}: the chosen {tap:?} was not tapped"
                            );
                        }
                    }
                    PayChoice::Auto => tally.automatic += 1,
                    PayChoice::Manual(_) => tally.manual += 1,
                }
            }
            previous = Some(decision);
        }
    }
    assert!(
        tally.cancelled > 0 && tally.completed > 0 && tally.automatic > 0 && tally.manual > 0,
        "every way to answer a payment should occur: {tally:?}"
    );
}

/// After a cancel: no events, same state, same log, the previous decision again, and a rebuilt engine agrees.
fn cancel_leaves_no_trace(
    setup: &mtg_engine::GameSetup,
    engine: &Engine,
    events: &[Event],
    (hash, log): (u64, usize),
    before: Option<&Decision>,
) {
    assert!(events.is_empty(), "a cancelled cast logs nothing");
    assert_eq!(engine.state_hash(), hash, "the game is as it was");
    assert_eq!(engine.events().len(), log);
    assert_eq!(
        engine.decision(),
        before,
        "the caster has the same priority decision again"
    );
    let rebuilt = Engine::resume(setup.clone(), engine.events().to_vec()).expect("log replays");
    assert_eq!(
        rebuilt.state_hash(),
        hash,
        "replaying the log gives the same state"
    );
    assert_eq!(rebuilt.decision(), engine.decision());
}
