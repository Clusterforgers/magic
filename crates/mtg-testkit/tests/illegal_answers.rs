//! Goal: no answer the engine rejects ever changes state, the log or the pending decision, whatever the decision kind and however the answer is malformed.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, MulliganChoice, PayChoice, PriorityAction};
use mtg_engine::mana::{ManaSource, Payment};
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{RandomBot, fuzz_setup, symbol_fuzz_setup};
use proptest::prelude::*;
use rstest::rstest;

fn object() -> impl Strategy<Value = ObjectId> {
    prop_oneof![9 => (0u64..130).prop_map(ObjectId), 1 => any::<u64>().prop_map(ObjectId)]
}

fn target() -> impl Strategy<Value = AttackTarget> {
    prop_oneof![
        (0usize..5).prop_map(|p| AttackTarget::Player(PlayerId::from_index(p))),
        object().prop_map(AttackTarget::Planeswalker),
    ]
}

fn mana() -> impl Strategy<Value = ManaType> {
    (0usize..6).prop_map(|i| ManaType::ALL[i])
}

fn source() -> impl Strategy<Value = ManaSource> {
    (object(), 0usize..3).prop_map(|(object, ability)| ManaSource { object, ability })
}

fn any_answer() -> impl Strategy<Value = Answer> {
    prop_oneof![
        prop_oneof![Just(MulliganChoice::Keep), Just(MulliganChoice::Mulligan)]
            .prop_map(Answer::Mulligan),
        prop::collection::vec(object(), 0..4).prop_map(Answer::BottomCards),
        prop::collection::vec(object(), 0..4).prop_map(Answer::Discard),
        prop_oneof![
            Just(PriorityAction::Pass),
            object().prop_map(PriorityAction::PlayLand),
            object().prop_map(PriorityAction::CastSpell),
            source().prop_map(PriorityAction::ActivateMana),
        ]
        .prop_map(Answer::Priority),
        prop_oneof![
            Just(PayChoice::Auto),
            Just(PayChoice::Cancel),
            (
                prop::collection::vec(mana(), 0..3),
                prop::collection::vec(source(), 0..4),
                0u32..7
            )
                .prop_map(|(pool, taps, life)| PayChoice::Manual(Payment {
                    pool,
                    taps,
                    life
                })),
            (
                prop::collection::vec(mana(), 0..3),
                prop::collection::vec(source(), 0..4),
                0u32..7
            )
                .prop_map(|(pool, taps, life)| PayChoice::Complete(Payment {
                    pool,
                    taps,
                    life
                })),
        ]
        .prop_map(Answer::PayMana),
        prop::collection::vec((object(), target()), 0..4).prop_map(Answer::DeclareAttackers),
        prop::collection::vec((object(), object()), 0..4).prop_map(Answer::DeclareBlockers),
        prop::collection::vec((object(), 0u32..8), 0..4).prop_map(Answer::AssignDamage),
    ]
}

/// Plays `steps` bot answers into a three-player game so the pending decision varies widely.
fn game_after(seed: u64, steps: usize) -> Result<Engine, EngineError> {
    let mut engine = Engine::new(fuzz_setup(3, seed))?;
    let mut bots: Vec<RandomBot> = (0..3).map(|i| RandomBot::new(seed, i)).collect();
    play_steps(&mut engine, &mut bots, steps)?;
    Ok(engine)
}

/// Like `game_after`, in a game of hybrid and Phyrexian costs played by bots that pay them by hand, so payments with life are pending too.
fn symbol_game_after(seed: u64, steps: usize) -> Result<Engine, EngineError> {
    let mut engine = Engine::new(symbol_fuzz_setup(3, seed))?;
    let mut bots: Vec<RandomBot> = (0..3)
        .map(|i| {
            RandomBot::new(seed, i)
                .with_symbol_choices()
                .with_partial_payments()
        })
        .collect();
    play_steps(&mut engine, &mut bots, steps)?;
    Ok(engine)
}

fn play_steps(
    engine: &mut Engine,
    bots: &mut [RandomBot],
    steps: usize,
) -> Result<(), EngineError> {
    for _ in 0..steps {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let answer = bots[decision.player.index()].choose(&decision);
        engine.apply(answer)?;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn rejected_answers_never_change_anything(
        seed in 0u64..40,
        steps in 0usize..400,
        answers in prop::collection::vec(any_answer(), 1..8),
    ) {
        let mut engine = game_after(seed, steps).map_err(|e| TestCaseError::fail(e.to_string()))?;
        for answer in answers {
            let (hash, log_len, decision) = (engine.state_hash(), engine.events().len(), engine.decision().cloned());
            match engine.apply(answer) {
                Ok(_) => {}
                Err(error) => {
                    prop_assert!(matches!(error, EngineError::Illegal(_) | EngineError::NoDecision), "{error:?}");
                    prop_assert_eq!(engine.state_hash(), hash);
                    prop_assert_eq!(engine.events().len(), log_len);
                    prop_assert_eq!(engine.decision().cloned(), decision);
                }
            }
        }
    }

    #[test]
    fn rejected_answers_never_change_anything_in_a_game_of_hybrid_and_phyrexian_costs(
        seed in 0u64..40,
        steps in 0usize..500,
        answers in prop::collection::vec(any_answer(), 1..8),
    ) {
        let mut engine = symbol_game_after(seed, steps).map_err(|e| TestCaseError::fail(e.to_string()))?;
        for answer in answers {
            let (hash, log_len, decision) = (engine.state_hash(), engine.events().len(), engine.decision().cloned());
            if let Err(error) = engine.apply(answer) {
                prop_assert!(matches!(error, EngineError::Illegal(_) | EngineError::NoDecision), "{error:?}");
                prop_assert_eq!(engine.state_hash(), hash);
                prop_assert_eq!(engine.events().len(), log_len);
                prop_assert_eq!(engine.decision().cloned(), decision);
            }
        }
    }

    #[test]
    fn accepted_answers_always_extend_the_log_consistently(seed in 0u64..40, steps in 0usize..300) {
        let mut engine = game_after(seed, steps).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let before = engine.events().len();
        if let Some(decision) = engine.decision().cloned() {
            let answer = RandomBot::new(seed, 99).choose(&decision);
            let new_events = engine.apply(answer).expect("legal answer");
            prop_assert_eq!(&engine.events()[before..], new_events.as_slice());
            prop_assert_eq!(mtg_engine::replay(engine.setup(), engine.events()).expect("replays").state_hash(), engine.state_hash());
        }
    }
}

#[rstest]
#[case::mulligan_wants_keep_or_mulligan(0, Answer::Priority(PriorityAction::Pass))]
#[case::mulligan_rejects_attackers(0, Answer::DeclareAttackers(vec![]))]
#[case::mulligan_rejects_discards(0, Answer::Discard(vec![]))]
fn wrong_kinds_of_answer_are_rejected_at_the_opening(#[case] steps: usize, #[case] answer: Answer) {
    let mut engine = game_after(1, steps).expect("legal bot play");
    let (hash, log_len) = (engine.state_hash(), engine.events().len());
    assert!(matches!(engine.apply(answer), Err(EngineError::Illegal(_))));
    assert_eq!(
        (engine.state_hash(), engine.events().len()),
        (hash, log_len)
    );
}

#[test]
fn no_decision_means_no_answers_and_no_effects() {
    let mut engine = Engine::new(
        mtg_testkit::Scenario::new(2)
            .life(0, 0)
            .setup()
            .expect("setup"),
    )
    .expect("engine");
    let hash = engine.state_hash();
    assert_eq!(
        engine.apply(Answer::Priority(PriorityAction::Pass)),
        Err(EngineError::NoDecision)
    );
    assert_eq!(engine.state_hash(), hash);
    assert!(
        engine
            .events()
            .iter()
            .any(|e| matches!(e, Event::GameEnded { .. }))
    );
}
