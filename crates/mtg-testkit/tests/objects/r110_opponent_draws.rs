//! Goal: "unless any opponent has you draw a card", repeated (Development), at 2 to 4 seats: each opponent is asked in
//! turn, one who agrees makes the caster draw instead of the token, and with nobody agreeing every round makes a token;
//! Research finds nothing outside the game; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay};
use crate::common::copying::replays;
use crate::common::{names, pid};

const CARD: &str = "Research // Development";

fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(CARD).unwrap()])
        .library(0, (0..5).map(|_| forest()).collect::<Vec<_>>())
        .battlefield(
            0,
            [
                ready(forest()),
                ready(island()),
                ready(island()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.library(seat, [forest(), forest()]);
    }
    scenario
}

/// Casts the face named, answering each opponent's yes or no by `agrees(seat, round)`; returns how many questions
/// each opponent was asked.
fn play(
    driver: &mut Driver,
    face: &str,
    agrees: impl Fn(usize, usize) -> bool,
    seats: usize,
) -> Result<Vec<usize>, ScriptError> {
    driver.begin_cast(CARD)?;
    choose_way(driver, face)?;
    pay(driver)?;
    let mut asked = vec![0; seats];
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::YesNo(_) if decision.player != pid(0) => {
                let seat = decision.player.index();
                let yes = agrees(seat, asked[seat]);
                asked[seat] += 1;
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn elementals(driver: &Driver) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Elemental")
        .count()
}

#[rstest]
#[case::nobody_agrees(None, 3, 0)]
#[case::the_last_opponent_agrees_every_time(Some(usize::MAX), 0, 3)]
fn development_makes_a_token_unless_an_opponent_has_you_draw(
    #[values(2, 3, 4)] seats: usize,
    #[case] agreeing: Option<usize>,
    #[case] tokens: usize,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let agreeing = agreeing.map(|seat| seat.min(seats - 1));
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let asked = play(
        &mut driver,
        "Development",
        |seat, _| Some(seat) == agreeing,
        seats,
    )?;
    assert_eq!(elementals(&driver), tokens);
    // The spell left the hand; each agreement drew a card.
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand - 1 + drawn
    );
    // Every opponent is asked each round until one agrees.
    for seat in 1..seats {
        let expected = match agreeing {
            Some(yes) if seat > yes => 0,
            _ => 3,
        };
        assert_eq!(asked[seat], expected, "seat {seat}");
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn research_finds_nothing_outside_the_game(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let library = driver.state().zone(Zone::Library(pid(0))).len();
    play(&mut driver, "Research", |_, _| false, seats)?;
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), library);
    replays(&driver, &setup);
    Ok(())
}
