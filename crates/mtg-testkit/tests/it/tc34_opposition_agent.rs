//! Goal: under Opposition Agent an opponent's library search is answered by the Agent's controller and every card it finds is exiled instead of reaching its destination; the Agent's controller may then play those cards from exile while they stay there, paying with mana of any color, the searcher may not, the Agent's controller's own searches are untouched, at tables of two, three and four seats, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, pid};
use mtg_core::color::Color;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::control_cards::tutor;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 has Opposition Agent and a Mountain; every seat holds a tutor and has a library of three green creatures for {G}.
fn table(seats: usize, active: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(1, active, Step::Main1).battlefield(
        0,
        [ready(card("Opposition Agent").unwrap()), ready(mountain())],
    );
    for seat in 0..seats {
        scenario = scenario
            .hand(seat, vec![tutor(&format!("Tutor {seat}"))])
            .library(
                seat,
                (0..3).map(|n| creature(&format!("Card {seat}.{n}"), 0, &[Color::Green], 1, 1)),
            );
    }
    scenario
}

/// Casts the tutor of `seat`, has whoever answers the search take the first card offered and lets it resolve; the
/// player who answered.
fn search(driver: &mut Driver, seat: usize) -> Result<usize, ScriptError> {
    driver.cast(&format!("Tutor {seat}"))?;
    let mut answerer = None;
    for _ in 0..60 {
        if answerer.is_some()
            && driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
        {
            break;
        }
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind
            && answerer.is_none()
        {
            answerer = Some(decision.player.index());
            driver.answer(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))?;
            continue;
        }
        driver.answer_passively()?;
    }
    answerer.ok_or_else(|| ScriptError::Stuck("the search".into()))
}

#[rstest]
fn an_opponent_exiles_what_they_find_and_the_agent_plays_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let searcher = seats - 1;
    let mut driver = Driver::from_scenario(table(seats, searcher))?;
    assert_eq!(search(&mut driver, searcher)?, 0);
    let exiled = names(driver.state(), Zone::Exile);
    assert_eq!(exiled.len(), 1, "{exiled:?}");
    assert!(exiled[0].starts_with(&format!("Card {searcher}.")));
    assert!(
        !names(driver.state(), Zone::Hand(pid(searcher)))
            .iter()
            .any(|name| name.starts_with("Card "))
    );
    // The searcher may not cast the exiled card.
    let found = exiled[0].clone();
    assert!(driver.cast(&found).is_err());
    // On the Agent's controller's turn, a Mountain pays the green card's {G}.
    // The searcher is the last seat, so seat 0's turn is the next one.
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(driver.state().turn().active(), pid(0));
    driver.cast_tapping(&found, &["Mountain"])?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(names(driver.state(), Zone::Battlefield).contains(&found));
    let id = driver.id_of(&found)?;
    assert_eq!(driver.state().object(id).unwrap().controller(), pid(0));
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn the_agents_controller_searches_as_usual(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 0))?;
    assert_eq!(search(&mut driver, 0)?, 0);
    assert!(names(driver.state(), Zone::Exile).is_empty());
    assert!(
        names(driver.state(), Zone::Hand(pid(0)))
            .iter()
            .any(|name| name.starts_with("Card 0."))
    );
    Ok(())
}
