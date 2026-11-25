//! Goal: B'Elanna Torres, Scrapper plays its Oracle text at tables of two, three and four seats: "{2}, {T}: Surveil 2,
//! then you may reveal the top card of your library. If an artifact card is revealed this way, draw a card. If you don't
//! draw a card this way, put two +1/+1 counters on B'Elanna Torres." An artifact kept on top and revealed is drawn; a
//! land on top, an artifact left unrevealed, or an artifact surveilled away gives the counters; the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{names, on_battlefield, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const BELANNA: &str = "B'Elanna Torres, Scrapper";

/// Resolves the activation: the surveil puts the first `binned` cards it shows into the graveyard and keeps the order of
/// the rest; the reveal is answered with `reveal`.
fn settle(driver: &mut Driver, binned: usize, reveal: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseCards(options) => {
                let chosen = options.from.iter().copied().take(binned).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(reveal)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::artifact_revealed_is_drawn(&["Sol Ring", "Forest"], 0, true, true)]
#[case::land_revealed_gives_counters(&["Forest", "Sol Ring"], 0, true, false)]
#[case::artifact_not_revealed_gives_counters(&["Sol Ring", "Forest"], 0, false, false)]
#[case::artifact_surveilled_away_gives_counters(&["Sol Ring", "Forest", "Forest"], 1, true, false)]
#[case::artifact_brought_up_by_the_surveil_is_drawn(&["Forest", "Sol Ring", "Forest"], 1, true, true)]
fn belanna_draws_an_artifact_revealed_or_grows(
    #[case] library: &[&str],
    #[case] binned: usize,
    #[case] reveal: bool,
    #[case] draws: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let deck = library
        .iter()
        .map(|name| {
            if *name == "Forest" {
                forest()
            } else {
                card(name).unwrap()
            }
        })
        .collect::<Vec<_>>();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card(BELANNA).unwrap()),
                ready(island()),
                ready(island()),
            ],
        )
        .library(0, deck);
    let mut driver = Driver::from_scenario(scenario)?;
    let hand_before = names(driver.state(), Zone::Hand(pid(0))).len();
    driver.begin_activation(BELANNA)?;
    if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    assert_eq!(
        driver.state().stack().len(),
        1,
        "the ability is on the stack"
    );
    settle(&mut driver, binned, reveal)?;

    let hand = names(driver.state(), Zone::Hand(pid(0)));
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(graveyard.len(), binned, "{graveyard:?}");
    let id = on_battlefield(driver.state(), BELANNA).unwrap();
    let object = driver.state().object(id).unwrap();
    let counters = object
        .permanent()
        .map_or(0, |held| held.counters_of(&CounterKind::PlusOnePlusOne));
    assert!(
        object
            .permanent()
            .is_some_and(mtg_engine::object::PermanentState::tapped)
    );
    if draws {
        assert_eq!(hand.len(), hand_before + 1);
        assert!(hand.iter().any(|name| name == "Sol Ring"), "{hand:?}");
        assert_eq!(counters, 0);
    } else {
        assert_eq!(hand.len(), hand_before);
        assert_eq!(counters, 2);
    }
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
