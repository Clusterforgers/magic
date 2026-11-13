//! Goal: "if this spell is the first spell you've cast this game, you may cast it without paying its mana cost" is open
//! at two to four seats only while its caster has cast no spell in the game, a spell cast on an earlier turn still
//! closing it, and Once Upon a Time cast that way puts a creature or land card from the top five into its caster's hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds Once Upon a Time and a free 1/1, with no land; every library holds bears.
fn r379_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1).hand(
        0,
        [
            card("Once Upon a Time").unwrap(),
            free_creature("Sprite", 1, 1),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| bear()));
    }
    scenario
}

/// Answers until the stack is empty and seat 0 has priority, taking the first card offered.
fn r379_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(options) if options.max > 0 => {
                let first = options.from[..1].to_vec();
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(first)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

#[rstest]
fn r379_the_first_spell_is_free(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r379_table(seats))?;
    let library = driver.state().zone(Zone::Library(pid(0))).len();
    driver.begin_cast("Once Upon a Time")?;
    r379_settle(&mut driver)?;
    let state = driver.state();
    assert!(
        state
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| state.object(*id).unwrap().card().name == "Once Upon a Time"),
        "it resolved"
    );
    let bears_in_hand = state
        .zone(Zone::Hand(pid(0)))
        .iter()
        .filter(|id| state.object(**id).unwrap().card().name == "Grizzly Bears")
        .count();
    assert_eq!(bears_in_hand, 1, "a creature card from the top five");
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), library - 1);
    let setup = r379_table(seats).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
#[case::near_miss_earlier_this_turn(false)]
#[case::near_miss_on_an_earlier_turn(true)]
fn r379_a_later_spell_is_not_free(
    #[values(2, 3, 4)] seats: usize,
    #[case] next_turn: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r379_table(seats))?;
    driver.cast("Sprite")?;
    r379_settle(&mut driver)?;
    if next_turn {
        let turn = driver.state().turn().number() + u32::try_from(seats).unwrap();
        driver.idle_until(turn, Step::Main1)?;
    }
    assert!(
        driver.begin_cast("Once Upon a Time").is_err(),
        "no land pays its mana cost, and it is no longer the first spell"
    );
    Ok(())
}
