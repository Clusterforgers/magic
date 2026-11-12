//! Goal: an engine rebuilt from the log at any decision of a game of triggers stands where the original stood, because the queue of triggers is state: at the order of two triggers and at the targets of one it asks the same question, and at a cast that was half announced it asks for priority again, as it always has.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_engine::{Engine, EngineError};
use mtg_testkit::{RandomBot, trigger_fuzz_setup};
use rstest::rstest;

#[rstest]
fn a_rebuilt_engine_asks_what_the_original_asks_about_triggers(
    #[values(2, 3)] players: usize,
) -> Result<(), EngineError> {
    let (mut orders, mut targets, mut casts) = (0, 0, 0);
    for seed in 0..8 {
        let setup = trigger_fuzz_setup(players, seed);
        let mut engine = Engine::new(setup.clone())?;
        let mut bots: Vec<RandomBot> = (0..players as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for step in 0..1500 {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            let resumed = Engine::resume(setup.clone(), engine.events().to_vec())?;
            assert_eq!(
                resumed.state_hash(),
                engine.state_hash(),
                "seed {seed} step {step}"
            );
            match &decision.kind {
                DecisionKind::OrderTriggers(_) => {
                    orders += 1;
                    assert_eq!(
                        resumed.decision(),
                        engine.decision(),
                        "seed {seed} step {step}"
                    );
                }
                DecisionKind::ChooseTargets(options) if options.triggered => {
                    targets += 1;
                    assert_eq!(
                        resumed.decision(),
                        engine.decision(),
                        "seed {seed} step {step}"
                    );
                }
                DecisionKind::ChooseTargets(_) | DecisionKind::PayMana(_) => {
                    // A half-finished cast lives outside game state, so a rebuilt engine asks for priority again.
                    casts += 1;
                    let again = resumed.decision().map(|d| (d.player, d.kind.name()));
                    assert_eq!(
                        again,
                        Some((decision.player, "priority")),
                        "seed {seed} step {step}"
                    );
                }
                _ => assert_eq!(
                    resumed.decision(),
                    engine.decision(),
                    "seed {seed} step {step}"
                ),
            }
            let answer = bots[decision.player.index()].choose(&decision);
            engine.apply(answer)?;
        }
    }
    assert!(orders > 0, "no trigger order was asked");
    assert!(targets > 0, "no trigger target was asked");
    assert!(casts > 0, "no cast was half announced");
    Ok(())
}
