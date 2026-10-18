//! Goal: every `FormatRules` hook (life, deck validation, mulligan, zones, loss and result) really drives the engine, so Commander can plug in without engine changes.

mod common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice, MulliganOptions};
use mtg_engine::error::SetupError;
use mtg_engine::format::{DeckError, ExtraZone, FormatRules, MulliganRule, Normal};
use mtg_engine::player::LossReason;
use mtg_engine::state::GameResult;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameSetup, GameState};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, fuzz_setup};
use rstest::rstest;

/// A deliberately odd format: 40 life, 5-card hands with one free mulligan, tiny decks, a
/// custom loss condition and a draw when only two players are left.
#[derive(Debug)]
struct Quirky;

impl FormatRules for Quirky {
    fn name(&self) -> &'static str {
        "quirky"
    }

    fn starting_life(&self) -> i32 {
        40
    }

    fn validate_deck(&self, deck: &[Arc<CardDefinition>]) -> Result<(), DeckError> {
        if deck.len() < 10 {
            return Err(DeckError::TooFewCards {
                min: 10,
                got: deck.len(),
            });
        }
        Ok(())
    }

    fn mulligan_rule(&self) -> MulliganRule {
        MulliganRule {
            hand_size: 5,
            free_mulligans: 1,
        }
    }

    fn extra_zones(&self) -> &'static [ExtraZone] {
        &[ExtraZone::Command]
    }

    fn skips_first_draw(&self, _player_count: usize) -> bool {
        true
    }

    fn loss_condition(&self, state: &GameState, player: PlayerId) -> Option<LossReason> {
        (state.player(player)?.life() == 13).then(|| LossReason::Format("unlucky thirteen".into()))
    }

    fn game_result(&self, state: &GameState) -> Option<GameResult> {
        (state.alive_players().count() <= 2).then_some(GameResult::Draw)
    }
}

fn quirky_decks(players: usize, size: usize) -> Vec<Vec<Arc<CardDefinition>>> {
    (0..players)
        .map(|_| (0..size).map(|_| forest()).collect())
        .collect()
}

#[test]
fn starting_life_and_hand_size_come_from_the_format() -> Result<(), ScriptError> {
    let engine = Engine::new(GameSetup::new(Arc::new(Quirky), 3, quirky_decks(3, 12)))?;
    assert!(engine.state().players().iter().all(|p| p.life() == 40));
    let DecisionKind::Mulligan(options) = &engine.decision().ok_or(EngineError::NoDecision)?.kind
    else {
        return Err(ScriptError::UnexpectedDecision {
            wanted: "mulligan",
            got: "other".into(),
        });
    };
    assert_eq!(options.hand.len(), 5);
    assert_eq!(engine.state().format().name(), "quirky");
    assert_eq!(engine.state().format().extra_zones(), &[ExtraZone::Command]);
    Ok(())
}

#[test]
fn normal_is_twenty_life_and_seven_cards() -> Result<(), ScriptError> {
    let engine = Engine::new(fuzz_setup(2, 1))?;
    assert!(engine.state().players().iter().all(|p| p.life() == 20));
    assert_eq!(engine.state().zone(Zone::Hand(pid(0))).len(), 7);
    Ok(())
}

/// The keep-or-mulligan options the driver is waiting on.
fn mulligan_options(driver: &Driver) -> Result<MulliganOptions, ScriptError> {
    match &driver.decision()?.kind {
        DecisionKind::Mulligan(options) => Ok(options.clone()),
        _ => Err(ScriptError::UnexpectedDecision {
            wanted: "mulligan",
            got: "other".into(),
        }),
    }
}

#[rstest]
#[case::free_first_mulligan(1, 0)]
#[case::second_costs_one(2, 1)]
#[case::third_costs_two(3, 2)]
fn free_mulligans_reduce_the_cards_bottomed(
    #[case] mulligans: u32,
    #[case] bottom: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::new(Engine::new(GameSetup::new(
        Arc::new(Quirky),
        5,
        quirky_decks(3, 20),
    ))?)?;
    let first = driver.state().first_player();
    let mut offered = Vec::new();
    for round in 0..mulligans {
        offered.push(mulligan_options(&driver)?);
        driver.answer(Answer::Mulligan(MulliganChoice::Mulligan))?;
        if round == 0 {
            driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
            driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
        }
    }
    let last = mulligan_options(&driver)?;
    driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    match &driver.decision()?.kind {
        DecisionKind::BottomCards(options) => {
            assert_eq!((options.count, driver.decision()?.player), (bottom, first));
        }
        _ => assert_eq!(
            bottom, 0,
            "no bottom decision only when nothing needs bottoming"
        ),
    }
    // The options the player saw told the truth: keeping bottoms what the next decision asks
    // for, and the price shown for a mulligan is what the next hand's options then show.
    assert_eq!((last.free_mulligans, last.bottom_if_keep), (1, bottom));
    offered.push(last);
    for pair in offered.windows(2) {
        assert_eq!(pair[0].bottom_if_mulligan, pair[1].bottom_if_keep);
    }
    Ok(())
}

#[test]
fn normal_mulligans_are_never_free() -> Result<(), ScriptError> {
    let driver = Driver::new(Engine::new(fuzz_setup(2, 1))?)?;
    let options = mulligan_options(&driver)?;
    assert_eq!(
        (
            options.free_mulligans,
            options.bottom_if_keep,
            options.bottom_if_mulligan
        ),
        (0, 0, 1)
    );
    Ok(())
}

#[rstest]
#[case::too_small(9, false)]
#[case::just_enough(10, true)]
fn deck_validation_is_the_formats(#[case] size: usize, #[case] legal: bool) {
    let result = Engine::new(GameSetup::new(Arc::new(Quirky), 1, quirky_decks(2, size)));
    assert_eq!(result.is_ok(), legal);
    if !legal {
        assert!(matches!(
            result.err(),
            Some(EngineError::Setup(SetupError::InvalidDeck {
                source: DeckError::TooFewCards { min: 10, got: 9 },
                ..
            }))
        ));
    }
}

#[rstest]
#[case::normal_too_few(vec![59, 60], "TooFewCards")]
#[case::normal_second_deck_bad(vec![60, 12], "TooFewCards")]
fn normal_rejects_bad_decks_at_setup(#[case] sizes: Vec<usize>, #[case] error: &str) {
    let decks = sizes
        .iter()
        .map(|n| (0..*n).map(|_| forest()).collect())
        .collect();
    let result = Engine::new(GameSetup::new(Arc::new(Normal), 1, decks));
    assert!(format!("{:?}", result.err()).contains(error));
}

#[test]
fn normal_enforces_the_copy_limit_but_not_for_basic_lands() {
    let mut deck: Vec<Arc<CardDefinition>> = (0..55).map(|_| forest()).collect();
    deck.extend((0..5).map(|_| free_creature("Same Name", 1, 1)));
    let decks = vec![deck.clone(), deck];
    let result = Engine::new(GameSetup::new(Arc::new(Normal), 1, decks));
    assert!(matches!(
        result.err(),
        Some(EngineError::Setup(SetupError::InvalidDeck {
            source: DeckError::TooManyCopies { got: 5, max: 4, .. },
            ..
        }))
    ));
}

#[test]
fn games_need_at_least_two_players() {
    let result = Engine::new(GameSetup::new(
        Arc::new(Normal),
        1,
        vec![quirky_decks(1, 60).remove(0)],
    ));
    assert_eq!(
        result.err(),
        Some(EngineError::Setup(SetupError::TooFewPlayers(1)))
    );
}

#[test]
fn a_format_loss_condition_eliminates_players() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(4)
            .format(Arc::new(Quirky))
            .life(2, 13)
            .life(0, 12),
    )?;
    assert!(!driver.state().is_alive(pid(2)));
    assert!(driver.state().is_alive(pid(0)));
    driver.expect_event("format loss", |e| {
        matches!(e, Event::PlayerLost { player, reason: LossReason::Format(why) } if *player == pid(2) && why == "unlucky thirteen")
    })
}

#[test]
fn a_format_can_decide_the_game_result_itself() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(4)
            .format(Arc::new(Quirky))
            .life(2, 0)
            .life(3, 0),
    )?;
    assert_eq!(
        driver.engine().result(),
        Some(GameResult::Draw),
        "two players left is a draw in this format"
    );
    Ok(())
}
