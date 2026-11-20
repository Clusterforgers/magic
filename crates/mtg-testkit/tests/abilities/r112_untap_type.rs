//! Goal: an untap step that untaps one chosen card type (CR 502.3) at 2 to 4 seats: under an untapped Storage Matrix the
//! active player chooses artifact, creature or land before anything untaps and only permanents of that type untap; a
//! tapped Matrix asks nothing and everything untaps; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

use crate::common::copying::replays;

/// Turn 1 ends with seat 0 active; seat 1, next, controls the Matrix (untapped or not), a tapped Grizzly Bears, a tapped
/// Forest and a tapped Mind Stone.
fn table(seats: usize, matrix_tapped: bool) -> Scenario {
    let matrix = card("Storage Matrix").unwrap();
    let mut scenario = Scenario::new(seats).at(1, 0, Step::End).battlefield(
        1,
        [
            if matrix_tapped {
                tapped(matrix)
            } else {
                ready(matrix)
            },
            tapped(card("Grizzly Bears").unwrap()),
            tapped(forest()),
            tapped(card("Mind Stone").unwrap()),
        ],
    );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| plains()));
    }
    scenario
}

/// Plays to seat 1's upkeep, answering the untap question with `pick`; returns how often it was asked and of whom.
fn to_upkeep(driver: &mut Driver, pick: usize) -> Result<Vec<usize>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        if driver.state().turn().number() == 2 && driver.state().turn().step() == Step::Upkeep {
            return Ok(asked);
        }
        if let DecisionKind::ChooseOption(options) = &decision.kind {
            assert_eq!(options.options.len(), 3);
            asked.push(decision.player.index());
            driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("seat 1's upkeep".into()))
}

fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

#[rstest]
#[case::creatures(1, [false, true, true])]
#[case::lands(2, [true, false, true])]
#[case::artifacts(0, [true, true, false])]
fn only_the_chosen_type_untaps(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
    #[case] still: [bool; 3],
) -> Result<(), ScriptError> {
    let setup = table(seats, false).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, false))?;
    assert_eq!(to_upkeep(&mut driver, pick)?, [1]);
    let names = ["Grizzly Bears", "Forest", "Mind Stone"];
    let now: Vec<bool> = names.iter().map(|name| is_tapped(&driver, name)).collect();
    assert_eq!(now, still);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_tapped_matrix_asks_nothing(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, true))?;
    assert_eq!(to_upkeep(&mut driver, 0)?, Vec::<usize>::new());
    for name in ["Storage Matrix", "Grizzly Bears", "Forest", "Mind Stone"] {
        assert!(!is_tapped(&driver, name), "{name}");
    }
    Ok(())
}
