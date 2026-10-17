//! Goal: the opening procedure deals seven, lets every player keep or London-mulligan in turn order, and starts turn one only when all have kept.

mod common;

use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice};
use mtg_engine::player::MulliganStatus;
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::{fuzz_setup, Driver, ScriptError};
use rstest::rstest;

fn opening(players: usize, seed: u64) -> Result<Driver, ScriptError> {
    Driver::new(Engine::new(fuzz_setup(players, seed))?)
}

fn mulligan_decision(driver: &Driver) -> Result<(usize, Vec<ObjectId>, u32, bool), ScriptError> {
    let decision = driver.decision()?;
    match &decision.kind {
        DecisionKind::Mulligan(o) => Ok((decision.player.index(), o.hand.clone(), o.mulligans_taken, o.can_mulligan)),
        other => Err(ScriptError::UnexpectedDecision { wanted: "mulligan", got: other.name().to_owned() }),
    }
}

fn decide(driver: &mut Driver, choice: MulliganChoice) -> Result<(), ScriptError> {
    driver.answer(Answer::Mulligan(choice))?;
    Ok(())
}

fn keep_everyone(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Mulligan(_)) {
        decide(driver, MulliganChoice::Keep)?;
    }
    Ok(())
}

#[rstest]
fn every_player_is_dealt_seven_and_asked_in_turn_order(
    #[values(2, 3, 5, 8)] players: usize,
    #[values(1, 2, 3)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = opening(players, seed)?;
    let first = driver.state().first_player().index();
    let mut asked = Vec::new();
    while matches!(driver.decision()?.kind, DecisionKind::Mulligan(_)) {
        let (seat, hand, taken, can) = mulligan_decision(&driver)?;
        assert_eq!((hand.len(), taken, can), (7, 0, true));
        asked.push(seat);
        decide(&mut driver, MulliganChoice::Keep)?;
    }
    let expected: Vec<usize> = (0..players).map(|i| (first + i) % players).collect();
    assert_eq!(asked, expected, "the starting player decides first, then turn order");
    Ok(())
}

#[test]
fn the_first_player_is_random_but_seeded() -> Result<(), ScriptError> {
    let mut seen = [false; 3];
    for seed in 0..60 {
        let first = opening(3, seed)?.state().first_player().index();
        assert_eq!(first, opening(3, seed)?.state().first_player().index(), "same seed, same first player");
        seen[first] = true;
    }
    assert_eq!(seen, [true, true, true], "every seat can go first");
    Ok(())
}

#[rstest]
#[case(2, 7, false)]
#[case(3, 8, true)]
#[case(6, 8, true)]
fn keeping_everywhere_starts_turn_one_with_the_first_player(
    #[case] players: usize,
    #[case] first_hand: usize,
    #[case] draws: bool,
) -> Result<(), ScriptError> {
    let mut driver = opening(players, 4)?;
    keep_everyone(&mut driver)?;
    let state = driver.state();
    assert_eq!(state.stage(), Stage::Turns);
    assert_eq!(state.turn().number(), 1);
    assert_eq!(state.turn().active(), state.first_player());
    driver.expect_decision(state.first_player().index(), "priority")?;
    driver.idle_until(1, Step::Main1)?;
    let hand = driver.state().zone(Zone::Hand(driver.state().first_player())).len();
    assert_eq!(hand, first_hand);
    assert_eq!(draws, hand == 8);
    Ok(())
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(2)]
#[case(3)]
#[case(5)]
fn london_mulligan_draws_seven_then_bottoms_one_per_mulligan(#[case] mulligans: u32) -> Result<(), ScriptError> {
    let mut driver = opening(2, 11)?;
    let player = driver.state().first_player();
    for round in 0..mulligans {
        let (seat, hand, taken, can) = mulligan_decision(&driver)?;
        assert_eq!((seat, hand.len(), taken, can), (player.index(), 7, round, true));
        decide(&mut driver, MulliganChoice::Mulligan)?;
        // The other player keeps in the first round only; afterwards only mulliganers are asked.
        if round == 0 {
            decide(&mut driver, MulliganChoice::Keep)?;
        }
    }
    let (seat, hand, taken, _) = mulligan_decision(&driver)?;
    assert_eq!((seat, hand.len(), taken), (player.index(), 7, mulligans));
    decide(&mut driver, MulliganChoice::Keep)?;
    if mulligans == 0 {
        assert!(
            !matches!(driver.decision()?.kind, DecisionKind::BottomCards(_)),
            "no cards to bottom after zero mulligans"
        );
    } else {
        let DecisionKind::BottomCards(options) = &driver.decision()?.kind else {
            return Err(ScriptError::UnexpectedDecision { wanted: "bottom-cards", got: "something else".into() });
        };
        assert_eq!(options.count, mulligans as usize);
        let bottom: Vec<ObjectId> = options.hand.iter().rev().copied().take(mulligans as usize).collect();
        driver.answer(Answer::BottomCards(bottom.clone()))?;
        let state = driver.state();
        let library = state.zone(Zone::Library(player));
        assert_eq!(library.len(), 60 - 7 + mulligans as usize);
        let bottom_up: Vec<ObjectId> = bottom.iter().rev().copied().collect();
        assert_eq!(&library[..bottom.len()], bottom_up.as_slice(), "the last card named ends up on the very bottom");
        assert_eq!(state.zone(Zone::Hand(player)).len(), 7 - mulligans as usize);
    }
    assert_eq!(driver.state().player(player).map(|p| p.mulligans()), Some(mulligans));
    assert_eq!(driver.state().player(player).map(|p| p.mulligan_status()), Some(MulliganStatus::Kept));
    Ok(())
}

#[test]
fn all_players_declare_before_any_mulligan_happens() -> Result<(), ScriptError> {
    let mut driver = opening(3, 5)?;
    let order: Vec<usize> = (0..3).map(|i| (driver.state().first_player().index() + i) % 3).collect();
    decide(&mut driver, MulliganChoice::Mulligan)?;
    decide(&mut driver, MulliganChoice::Keep)?;
    decide(&mut driver, MulliganChoice::Mulligan)?;
    let log = driver.engine().events();
    let position = |pred: &dyn Fn(&Event) -> bool| log.iter().position(pred);
    let declared_first = position(&|e| matches!(e, Event::MulliganDeclared { player } if player.index() == order[0]));
    let kept_second = position(&|e| matches!(e, Event::MulliganKept { player } if player.index() == order[1]));
    let declared_third = position(&|e| matches!(e, Event::MulliganDeclared { player } if player.index() == order[2]));
    let first_taken = position(&|e| matches!(e, Event::MulliganTaken { .. }));
    assert!(declared_first < kept_second && kept_second < declared_third && declared_third < first_taken);
    assert_eq!(log.iter().filter(|e| matches!(e, Event::MulliganTaken { .. })).count(), 2);
    // The two mulliganers are asked again, in turn order; the keeper is not.
    let (seat, _, taken, _) = mulligan_decision(&driver)?;
    assert_eq!((seat, taken), (order[0], 1));
    decide(&mut driver, MulliganChoice::Keep)?;
    let DecisionKind::BottomCards(_) = &driver.decision()?.kind else {
        return Err(ScriptError::UnexpectedDecision { wanted: "bottom-cards", got: "other".into() });
    };
    Ok(())
}

#[test]
fn a_hand_of_zero_is_the_last_possible_mulligan() -> Result<(), ScriptError> {
    let mut driver = opening(2, 3)?;
    let player = driver.state().first_player();
    for round in 0..7 {
        decide(&mut driver, MulliganChoice::Mulligan)?;
        if round == 0 {
            decide(&mut driver, MulliganChoice::Keep)?;
        }
    }
    let (_, _, taken, can) = mulligan_decision(&driver)?;
    assert_eq!((taken, can), (7, false));
    let before = driver.engine().state_hash();
    assert!(matches!(
        driver.answer(Answer::Mulligan(MulliganChoice::Mulligan)),
        Err(ScriptError::Engine(EngineError::Illegal(_)))
    ));
    assert_eq!(driver.engine().state_hash(), before);
    decide(&mut driver, MulliganChoice::Keep)?;
    let DecisionKind::BottomCards(options) = &driver.decision()?.kind else {
        return Err(ScriptError::UnexpectedDecision { wanted: "bottom-cards", got: "other".into() });
    };
    let all = options.hand.clone();
    driver.answer(Answer::BottomCards(all))?;
    assert!(driver.state().zone(Zone::Hand(player)).is_empty());
    assert_eq!(driver.state().zone(Zone::Library(player)).len(), 60);
    Ok(())
}

#[test]
fn bad_bottom_choices_are_rejected_without_changes() -> Result<(), ScriptError> {
    let mut driver = opening(2, 8)?;
    decide(&mut driver, MulliganChoice::Mulligan)?;
    decide(&mut driver, MulliganChoice::Keep)?;
    decide(&mut driver, MulliganChoice::Keep)?;
    let DecisionKind::BottomCards(options) = &driver.decision()?.kind else {
        return Err(ScriptError::UnexpectedDecision { wanted: "bottom-cards", got: "other".into() });
    };
    let (hand, stranger) = (options.hand.clone(), ObjectId(u64::MAX));
    let before = driver.engine().state_hash();
    for bad in [vec![], vec![hand[0], hand[1]], vec![stranger], vec![hand[0], hand[0]]] {
        assert!(driver.answer(Answer::BottomCards(bad)).is_err());
        assert_eq!(driver.engine().state_hash(), before);
    }
    driver.answer(Answer::BottomCards(vec![hand[3]]))?;
    Ok(())
}

#[test]
fn a_mulligan_reshuffles_with_new_randomness_and_keeps_every_card() -> Result<(), ScriptError> {
    let mut driver = opening(2, 21)?;
    let player = driver.state().first_player();
    let (_, first_hand, _, _) = mulligan_decision(&driver)?;
    decide(&mut driver, MulliganChoice::Mulligan)?;
    decide(&mut driver, MulliganChoice::Keep)?;
    let (_, second_hand, _, _) = mulligan_decision(&driver)?;
    assert_ne!(first_hand, second_hand, "a fresh seven");
    let state = driver.state();
    let owned = state.zone(Zone::Hand(player)).len() + state.zone(Zone::Library(player)).len();
    assert_eq!(owned, 60, "no card was lost or duplicated by the shuffle");
    let shuffles = driver.engine().events().iter().filter(|e| matches!(e, Event::MulliganTaken { .. })).count();
    assert_eq!(shuffles, 1);
    Ok(())
}

#[test]
fn wrong_answer_kinds_are_rejected_during_the_opening() -> Result<(), ScriptError> {
    let mut driver = opening(2, 2)?;
    let before = driver.engine().state_hash();
    let result = driver.answer(Answer::BottomCards(vec![]));
    assert!(matches!(result, Err(ScriptError::Engine(EngineError::Illegal(_)))));
    assert_eq!(driver.engine().state_hash(), before);
    Ok(())
}
