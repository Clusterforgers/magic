//! Goal: the invariant checkers and the game harness catch the corruption they exist to catch and report failures with the seed and event log.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::combat::DamageTarget;
use mtg_engine::{Engine, Event, GameState};
use mtg_testkit::cards::{forest, free_creature, keyworded};
use mtg_testkit::{
    FailureKind, InvariantChecker, PlayOptions, RandomBot, Scenario, Violation, check_settled,
    check_state, damaged, fuzz_setup, play_game, play_game_with, ready,
};
use rstest::rstest;

fn engine_for(seed: u64) -> Result<Engine, mtg_engine::EngineError> {
    Engine::new(fuzz_setup(2, seed))
}

#[rstest]
#[case::fresh_scenario(Scenario::new(3))]
#[case::busy_board(Scenario::new(4).battlefield(1, [ready(forest()), ready(free_creature("X", 2, 2))]).hand(0, [forest()]))]
fn well_formed_states_pass_the_stateless_checks(#[case] scenario: Scenario) {
    let engine = scenario.build().map_err(|e| e.to_string());
    assert!(engine.is_ok());
    assert_eq!(engine.map(|e| check_state(e.state())), Ok(Ok(())));
}

#[test]
fn the_checker_follows_a_clean_log_event_by_event() -> Result<(), String> {
    let engine = engine_for(3).map_err(|e| e.to_string())?;
    let mut checker = InvariantChecker::new(engine.setup()).map_err(|e| e.to_string())?;
    checker
        .observe(engine.state(), engine.events())
        .map_err(|e| e.to_string())?;
    checker
        .check_full_replay(engine.state(), engine.events())
        .map_err(|e| e.to_string())
}

#[test]
fn a_live_state_that_differs_from_the_log_is_a_replay_violation() -> Result<(), String> {
    let (a, b) = (engine_for(1), engine_for(2));
    let (a, b) = (a.map_err(|e| e.to_string())?, b.map_err(|e| e.to_string())?);
    let mut checker = InvariantChecker::new(a.setup()).map_err(|e| e.to_string())?;
    let result = checker.observe(b.state(), a.events());
    assert!(
        matches!(result, Err(Violation::Replay { .. })),
        "{result:?}"
    );
    Ok(())
}

#[rstest]
#[case::draw_from_the_middle(Event::CardDrawn { player: PlayerId(0), object: ObjectId(3) })]
#[case::spend_mana_that_is_not_there(Event::ManaSpent { player: PlayerId(0), mana: vec![ManaType::Green], restricted: Vec::new() })]
#[case::pay_more_life_than_there_is(Event::LifePaid { player: PlayerId(0), amount: 21 })]
#[case::pay_no_life(Event::LifePaid { player: PlayerId(0), amount: 0 })]
#[case::unknown_object(Event::PermanentTapped { object: ObjectId(10_000) })]
#[case::damage_to_a_card_in_a_library(Event::DamageDealt { source: ObjectId(0), target: DamageTarget::Object(ObjectId(1)), amount: 1, deathtouch: false })]
fn events_that_cannot_follow_from_the_state_are_violations(
    #[case] event: Event,
) -> Result<(), String> {
    let engine = engine_for(4).map_err(|e| e.to_string())?;
    let mut checker = InvariantChecker::new(engine.setup()).map_err(|e| e.to_string())?;
    let mut log = engine.events().to_vec();
    log.push(event);
    let result = checker.observe(engine.state(), &log);
    assert!(matches!(result, Err(Violation::Fold(_))), "{result:?}");
    Ok(())
}

#[test]
fn a_dropped_event_is_noticed_by_the_hash_comparison() -> Result<(), String> {
    let engine = engine_for(6).map_err(|e| e.to_string())?;
    let mut checker = InvariantChecker::new(engine.setup()).map_err(|e| e.to_string())?;
    let mut log = engine.events().to_vec();
    let dropped = log
        .iter()
        .rposition(|e| matches!(e, Event::PermanentTapped { .. } | Event::HandDealt { .. }));
    let dropped = dropped.ok_or("no event to drop")?;
    log.remove(dropped);
    let result = checker.observe(engine.state(), &log);
    assert!(
        matches!(result, Err(Violation::Replay { .. } | Violation::Fold(_))),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn failures_carry_the_seed_and_the_event_log() {
    let failure = play_game_with(
        fuzz_setup(2, 77),
        77,
        PlayOptions {
            max_answers: 5,
            ..PlayOptions::default()
        },
    )
    .expect_err("five answers cannot finish a game");
    assert!(matches!(failure.kind, FailureKind::StepLimit(5)));
    let report = failure.to_string();
    assert!(report.contains("seed 77"), "{report}");
    assert!(report.contains("FirstPlayerChosen"), "the log is printed");
    assert_eq!(failure.events.len(), report.lines().count() - 2);
}

#[test]
fn finished_games_report_how_they_ended() -> Result<(), String> {
    let outcome = play_game(fuzz_setup(3, 12), 12).map_err(|f| f.to_string())?;
    assert!(outcome.answers > 0 && outcome.turns >= 1);
    assert!(matches!(
        outcome.events.last(),
        Some(Event::GameEnded { .. })
    ));
    Ok(())
}

#[test]
fn invalid_setups_fail_before_any_play() {
    let mut setup = fuzz_setup(2, 1);
    setup.players[0].library.truncate(10);
    let failure = play_game(setup, 1).expect_err("deck too small");
    assert!(matches!(failure.kind, FailureKind::Engine(_)));
    assert_eq!(failure.events.len(), 0);
}

#[test]
fn bots_are_reproducible_and_seed_sensitive() -> Result<(), String> {
    let engine = engine_for(9).map_err(|e| e.to_string())?;
    let decision = engine.decision().cloned().ok_or("no decision")?;
    let answer = |seed, stream| RandomBot::new(seed, stream).choose(&decision);
    assert_eq!(answer(1, 0), answer(1, 0));
    let varied = (0..40).map(|s| answer(s, 0)).collect::<Vec<_>>();
    assert!(
        varied.iter().any(|a| a != &varied[0]),
        "different seeds make different keep/mulligan choices"
    );
    Ok(())
}

#[rstest]
#[case::healthy(2, 1, false, true)]
#[case::one_short_of_lethal(2, 1, true, true)]
#[case::lethal_damage_left_on_the_battlefield(2, 2, false, false)]
#[case::indestructible_with_lethal_damage(2, 9, true, true)]
#[case::zero_toughness(0, 0, false, false)]
#[case::zero_toughness_is_not_stopped_by_indestructible(0, 0, true, false)]
fn a_state_with_a_creature_that_should_have_died_is_not_settled(
    #[case] toughness: i32,
    #[case] marked: u32,
    #[case] indestructible: bool,
    #[case] settled: bool,
) -> Result<(), String> {
    let keywords: &[Keyword] = if indestructible {
        &[Keyword::Indestructible]
    } else {
        &[]
    };
    let card = keyworded("Subject", 1, toughness, keywords);
    let setup = Scenario::new(2)
        .battlefield(0, [damaged(card, marked)])
        .setup()
        .map_err(|e| e.to_string())?;
    let state = GameState::from_setup(&setup).map_err(|e| e.to_string())?;
    let result = check_settled(&state);
    assert_eq!(result.is_ok(), settled, "{result:?}");
    if !settled {
        assert!(
            matches!(result, Err(Violation::Bookkeeping(_))),
            "{result:?}"
        );
    }
    Ok(())
}

#[test]
fn a_creature_that_dies_of_damage_and_comes_back_within_one_stretch_of_the_log_has_no_damage()
-> Result<(), String> {
    use mtg_engine::decision::{DecisionKind, PayChoice};
    use mtg_engine::zones::Zone;
    use mtg_engine::{Answer, choose::Target};
    use mtg_testkit::cards::{mountain, swamp};
    use mtg_testkit::corpus::card;
    use mtg_testkit::{Driver, ScriptError};

    let fail = |e: ScriptError| e.to_string();
    // A Bolt kills the Skeleton, whose owner then pays to bring it back from the graveyard: damage, a death, an
    // activation and an entry, all in the stretch of the log that is checked at once.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [
                    ready(card("Reassembling Skeleton").map_err(|e| e.to_string())?),
                    ready(swamp()),
                    ready(swamp()),
                ],
            )
            .battlefield(1, [ready(mountain())])
            .hand(1, [card("Lightning Bolt").map_err(|e| e.to_string())?]),
    )
    .map_err(fail)?;
    driver.pass().map_err(fail)?;
    let skeleton = driver.id_of("Reassembling Skeleton").map_err(fail)?;
    let at = Target::Object(driver.state().object(skeleton).unwrap().reference());
    driver.cast_at("Lightning Bolt", &[at]).map_err(fail)?;
    for _ in 0..2 {
        driver.pass().map_err(fail)?;
    }
    assert_eq!(
        driver.state().object(skeleton).unwrap().zone(),
        Zone::Graveyard(PlayerId(0))
    );
    driver
        .begin_activation("Reassembling Skeleton")
        .map_err(fail)?;
    driver
        .answer(Answer::PayMana(PayChoice::Auto))
        .map_err(fail)?;
    while !driver.state().stack().is_empty() {
        assert!(matches!(
            driver.decision().map_err(fail)?.kind,
            DecisionKind::Priority(_)
        ));
        driver.pass().map_err(fail)?;
    }
    assert_eq!(
        driver.state().object(skeleton).unwrap().zone(),
        Zone::Battlefield
    );
    let mut checker = InvariantChecker::new(driver.engine().setup()).map_err(|e| e.to_string())?;
    checker
        .observe(driver.state(), driver.engine().events())
        .map_err(|e| e.to_string())
}
