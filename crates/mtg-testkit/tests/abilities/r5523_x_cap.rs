//! Goal: "X can't be greater than the number of players in the game" caps the X Open the Way's caster may announce at
//! the number of players at two to four seats, below what the mana could pay, and the spell then puts that many land
//! cards from the top of its caster's library onto the battlefield tapped.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

#[rstest]
fn r5523_open_the_way_caps_x_at_the_players(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let library: Vec<_> = (0..6).flat_map(|_| [bear(), forest()]).collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Open the Way").unwrap()])
        .battlefield(0, (0..10).map(|_| ready(forest())))
        .library(0, library);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Open the Way")?;
    let DecisionKind::ChooseNumber(options) = driver.decision()?.kind.clone() else {
        panic!("X is announced");
    };
    let players = u32::try_from(seats).unwrap();
    assert_eq!(options.max, Some(players), "eight mana could pay more");
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(players)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..100 {
        let state = driver.state();
        if state.stack().is_empty() && state.resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let lands = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.owner() == PlayerId(0))
        })
        .count();
    assert_eq!(lands, 10 + seats, "X land cards entered");
    Ok(())
}
