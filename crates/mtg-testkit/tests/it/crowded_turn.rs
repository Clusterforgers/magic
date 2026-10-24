//! Goal: a turn on a crowded table (1,500 tokens attack 1,500 blockers, then a sweep clears every permanent) costs time in proportion to the table, not to its square or worse: the whole turn stays far inside a bound a quadratic scan per permanent would exceed by minutes, and the log of the turn replays to the same state hash.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::manual::{CommonToken, ManualAction, ManualToken};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario, ready};

/// The tokens and the blockers of the table.
const CROWD: usize = 1_500;

/// What the whole turn may take in a debug build: twenty times the 1.2 s it takes on an idle machine, and far less than the
/// minutes the square of the table costs.
const BOUND: Duration = Duration::from_secs(30);

/// Seat 0 holds the lands and the sweep for the end of the turn; seat 1 holds `CROWD` blockers that deal no damage.
fn table() -> GameSetup {
    let lands = (0..4).map(|_| ready(plains())).collect::<Vec<_>>();
    let blockers = (0..CROWD).map(|_| ready(free_creature("Foe", 0, 5)));
    Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand_named(0, ["Wrath of God"])
        .battlefield(0, lands)
        .battlefield(1, blockers.collect::<Vec<_>>())
        .setup()
        .unwrap()
}

fn answer(engine: &mut Engine, answer: Answer) {
    engine.apply(answer).unwrap();
}

/// Seat 0 makes the tokens, ten to an answer.
fn make_tokens(engine: &mut Engine) {
    for _ in 0..CROWD / 10 {
        let tokens = ManualAction::CreateToken {
            token: ManualToken::Common(CommonToken::Soldier),
            count: 10,
        };
        answer(engine, Answer::Priority(PriorityAction::Manual(tokens)));
    }
}

/// Passes through the rest of turn 1 and turn 2, so the tokens are no longer summoning sick on turn 3.
fn idle_to_turn_three(engine: &mut Engine) {
    while !(engine.state().turn().number() == 3 && engine.state().turn().step() == Step::Main1) {
        let decision = engine.decision().unwrap().clone();
        answer(engine, Driver::passive_answer(&decision));
    }
}

/// Everything attacks seat 1, every blocker blocks one attacker, and priority passes to the second main step.
fn combat(engine: &mut Engine) {
    while engine.state().turn().step() != Step::Main2 {
        let decision = engine.decision().unwrap().clone();
        let next = match &decision.kind {
            DecisionKind::DeclareAttackers(options) => Answer::DeclareAttackers(
                options
                    .attackers
                    .iter()
                    .map(|a| (a.object, AttackTarget::Player(PlayerId::from_index(1))))
                    .collect(),
            ),
            DecisionKind::DeclareBlockers(options) => {
                let attackers: BTreeSet<ObjectId> = options
                    .blockers
                    .iter()
                    .flat_map(|b| b.can_block.iter().copied())
                    .collect();
                assert_eq!(attackers.len(), CROWD, "every token attacks");
                Answer::DeclareBlockers(
                    options
                        .blockers
                        .iter()
                        .zip(&attackers)
                        .map(|(blocker, attacker)| (blocker.object, *attacker))
                        .collect(),
                )
            }
            _ => Driver::passive_answer(&decision),
        };
        answer(engine, next);
    }
}

/// Casts the sweep in the second main step and lets it resolve.
fn sweep(engine: &mut Engine) {
    let DecisionKind::Priority(options) = &engine.decision().unwrap().kind else {
        panic!("priority expected");
    };
    let wrath = options.castable[0];
    answer(engine, Answer::Priority(PriorityAction::CastSpell(wrath)));
    answer(engine, Answer::PayMana(PayChoice::Auto));
    answer(engine, Answer::Priority(PriorityAction::Pass));
    answer(engine, Answer::Priority(PriorityAction::Pass));
}

#[test]
fn a_turn_of_a_crowded_table_costs_time_in_proportion_to_the_table() {
    let setup = table();
    let started = Instant::now();
    let mut engine = Engine::new(setup.clone()).unwrap();
    make_tokens(&mut engine);
    idle_to_turn_three(&mut engine);
    let battlefield = engine.state().zone(Zone::Battlefield).len();
    combat(&mut engine);
    assert_eq!(engine.state().zone(Zone::Battlefield).len(), battlefield);
    sweep(&mut engine);
    assert_eq!(engine.state().zone(Zone::Battlefield).len(), 4);
    let took = started.elapsed();
    assert!(took < BOUND, "the turn took {took:?}");
    let replayed = Engine::resume(setup, engine.events().to_vec()).unwrap();
    assert_eq!(replayed.state_hash(), engine.state_hash());
}
