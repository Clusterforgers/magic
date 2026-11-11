//! Goal: a player who controls another answers every decision of that player during that player's next turn and no other, sees that player's hand and the cards that player searches, the control ends when the following turn begins, an extra turn asked for follows the controlled turn uncontrolled, a rule that controls opponents while they search hands over exactly the search, a spell whose control is gained resolves for its new controller with the targets it chose, at tables of two, three and four seats, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::CardsChoice;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::control_cards::{mindslave, search_watcher, spell_thief, tutor};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds the control spell; every seat holds a tutor and has a library of three creatures.
fn table(seats: usize, extra_turn: bool) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        let mut hand = vec![tutor(&format!("Tutor {seat}"))];
        if seat == 0 {
            hand.push(mindslave("Mindslave", extra_turn));
        }
        scenario = scenario.hand(seat, hand).library(
            seat,
            (0..3).map(|n| free_creature(&format!("Card {seat}.{n}"), 1, 1)),
        );
    }
    scenario
}

/// Answers passively until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Casts the tutor of `seat` and has whoever answers the search take the first card offered.
fn search(driver: &mut Driver, seat: usize) -> Result<usize, ScriptError> {
    driver.cast(&format!("Tutor {seat}"))?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            let answerer = decision.player.index();
            let owned = driver.engine().decision_owned().unwrap().player.index();
            assert_eq!(owned, seat);
            for card in &options.from {
                assert!(
                    driver.state().is_shown_to(decision.player, *card) || {
                        mtg_engine::sight::offered_to(driver.state(), decision.player)
                            .contains(card)
                    }
                );
            }
            driver.answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))?;
            settle(driver)?;
            return Ok(answerer);
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the search".into()))
}

#[rstest]
fn the_controller_answers_the_controlled_turn_and_no_other(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, false))?;
    let target = driver.target_player(1);
    driver.cast_at("Mindslave", &[target])?;
    settle(&mut driver)?;
    // Scheduled for seat 1's next turn, not this one.
    assert_eq!(driver.state().player_controller(pid(1)), pid(1));
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(driver.state().turn().active(), pid(1));
    assert_eq!(driver.state().player_controller(pid(1)), pid(0));
    assert_eq!(driver.decision()?.player, pid(0));
    assert_eq!(driver.engine().decision_owned().unwrap().player, pid(1));
    // The controller sees the controlled hand; nobody else does.
    assert!(driver.state().sees_for(pid(0), pid(1)));
    for seat in 2..seats {
        assert!(!driver.state().sees_for(pid(seat), pid(1)));
    }
    // Seat 0 casts seat 1's tutor and answers its search; the card goes to seat 1's hand.
    assert_eq!(search(&mut driver, 1)?, 0);
    assert!(
        names(driver.state(), Zone::Hand(pid(1)))
            .iter()
            .any(|name| name.starts_with("Card 1."))
    );
    // The next turn is not controlled.
    driver.idle_until(3, Step::Main1)?;
    assert_ne!(driver.state().turn().active(), pid(1));
    assert_eq!(driver.state().player_controller(pid(1)), pid(1));
    assert!(driver.state().player_controls().is_empty());
    let active = driver.state().turn().active();
    assert_eq!(driver.decision()?.player, active);
    replays(&driver);
    Ok(())
}

#[rstest]
fn an_extra_turn_follows_the_controlled_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, true))?;
    let target = driver.target_player(1);
    driver.cast_at("Mindslave", &[target])?;
    settle(&mut driver)?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(driver.decision()?.player, pid(0));
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(driver.state().turn().active(), pid(1));
    assert!(driver.state().turn().is_extra());
    assert_eq!(driver.state().player_controller(pid(1)), pid(1));
    assert_eq!(driver.decision()?.player, pid(1));
    replays(&driver);
    Ok(())
}

#[rstest]
fn a_rule_controls_opponents_while_they_search(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] searcher: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, false)
        .at(1, searcher, Step::Main1)
        .battlefield(0, [ready(search_watcher("Agent"))]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(driver.decision()?.player, pid(searcher));
    assert_eq!(search(&mut driver, searcher)?, 0);
    // Only the search was handed over.
    assert_eq!(driver.decision()?.player, pid(searcher));
    assert!(
        names(driver.state(), Zone::Hand(pid(searcher)))
            .iter()
            .any(|name| name.starts_with(&format!("Card {searcher}.")))
    );
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_creature_spell_enters_under_the_taker(true)]
#[case::an_instant_hits_the_new_target(false)]
fn a_spell_taken_resolves_for_its_new_controller(
    #[case] creature: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [free_creature("Ogre", 3, 3), burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())])
            .hand(1, [spell_thief("Thief")]),
    )?;
    if creature {
        driver.cast("Ogre")?;
    } else {
        let target = driver.target_player(1);
        driver.cast_at("Bolt", &[target])?;
    }
    let spell = driver.state().stack()[0];
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    driver.begin_cast("Thief")?;
    let reference = driver.state().object(spell).unwrap().reference();
    driver.choose_targets(&[mtg_engine::choose::Target::Object(reference)])?;
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            let target = driver.target_player(0);
            driver.choose_targets(&[target])?;
        } else if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        } else if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    if creature {
        let ogre = driver.id_of("Ogre")?;
        assert_eq!(controller_of(driver.state(), ogre), Some(pid(1)));
        assert_eq!(driver.state().object(ogre).unwrap().owner(), pid(0));
    } else {
        assert_eq!(driver.state().player(pid(0)).unwrap().life(), 17);
        assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20);
    }
    replays(&driver);
    Ok(())
}
