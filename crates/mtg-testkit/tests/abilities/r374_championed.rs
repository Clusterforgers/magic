//! Goal: a trigger on a creature championed with the card ("When a Faerie is championed with this creature, tap all lands
//! target player controls.") fires when the champion ability exiles a creature of the type and not when none is
//! championed, and the targeted player's lands alone become tapped, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{name_of, pid};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0's first main phase: it holds Mistbind Clique and controls four Islands and, with `faerie`, a Faerie; every other
/// seat controls two Islands.
fn table(seats: usize, faerie: bool) -> Scenario {
    let sprite = Arc::new(
        (*free_creature("Sprite", 1, 1))
            .clone()
            .with_subtype("Faerie"),
    );
    let mine: Vec<_> = (0..4)
        .map(|_| ready(island()))
        .chain(faerie.then(|| ready(sprite)))
        .collect();
    (1..seats).fold(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, vec![card("Mistbind Clique").unwrap()])
            .battlefield(0, mine),
        |scenario, seat| scenario.battlefield(seat, vec![ready(island()), ready(island())]),
    )
}

/// Answers until the stack is empty: the Faerie is championed and `target` is the player whose lands tap.
fn settle(driver: &mut Driver, target: usize) -> Result<bool, ScriptError> {
    let mut targeted = false;
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let chosen: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| name_of(driver.state(), *id) == "Sprite")
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                targeted = true;
                let at = driver.target_player(target);
                driver.choose_targets(&[at])?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => return Ok(targeted),
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn tapped_islands(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.controller() == pid(seat) && o.card().name == "Island")
        .filter(|o| {
            o.permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count()
}

#[rstest]
#[case::the_next_seat(1)]
#[case::the_last_seat(9)]
fn the_targeted_players_lands_tap(
    #[case] target: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let target = target.min(seats - 1);
    let mut driver = Driver::from_scenario(table(seats, true))?;
    driver.cast_tapping("Mistbind Clique", &["Island", "Island", "Island", "Island"])?;
    let asked = settle(&mut driver, target)?;
    assert!(asked, "the trigger asks for its target");
    assert!(
        driver
            .state()
            .zone(Zone::Exile)
            .iter()
            .any(|id| name_of(driver.state(), *id) == "Sprite")
    );
    for seat in 1..seats {
        assert_eq!(
            tapped_islands(&driver, seat),
            if seat == target { 2 } else { 0 },
            "seat {seat}"
        );
    }
    Ok(())
}

#[rstest]
fn nothing_championed_nothing_taps(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, false))?;
    driver.cast_tapping("Mistbind Clique", &["Island", "Island", "Island", "Island"])?;
    assert!(!settle(&mut driver, 1)?, "no trigger without a champion");
    assert!(
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .all(|id| name_of(driver.state(), *id) != "Mistbind Clique")
    );
    for seat in 1..seats {
        assert_eq!(tapped_islands(&driver, seat), 0);
    }
    Ok(())
}
