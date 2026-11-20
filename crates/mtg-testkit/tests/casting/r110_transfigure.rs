//! Goal: transfigure (CR 702.71) at 2 to 4 seats: Fleshwrither's ability costs {1}{B}{B} and its sacrifice, finds only a
//! creature card with its mana value (4) and puts it onto the battlefield, and is offered only as a sorcery.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

/// Seat 0 controls Fleshwrither and three Swamps; its library holds a creature of each mana value 3, 4 and 5.
fn game(seats: usize, turn: u32, active: usize) -> Result<Driver, ScriptError> {
    let library = vec![
        creature("Three", 2, &[Color::Black], 3, 3),
        creature("Four", 3, &[Color::Black], 4, 4),
        creature("Five", 4, &[Color::Black], 5, 5),
    ];
    let permanents = vec![
        ready(card("Fleshwrither").unwrap()),
        ready(swamp()),
        ready(swamp()),
        ready(swamp()),
    ];
    let mut scenario = Scenario::new(seats)
        .at(turn, active, Step::Main1)
        .library(0, library)
        .battlefield(0, permanents);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 8]);
    }
    Driver::from_scenario(scenario)
}

#[rstest]
fn transfigure_finds_a_creature_of_the_same_mana_value(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 0)?;
    driver.begin_activation("Fleshwrither")?;
    let mut offered: Vec<String> = Vec::new();
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(options) => {
                offered = options
                    .from
                    .iter()
                    .map(|id| name_of(driver.state(), *id))
                    .collect();
                let chosen: Vec<ObjectId> = options.from.iter().copied().take(1).collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::Priority(_) if driver.state().zone(Zone::Stack).is_empty() => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    assert_eq!(offered, ["Four"], "only the creature card of mana value 4");
    let battlefield = names(driver.state(), Zone::Battlefield);
    assert!(battlefield.contains(&"Four".to_owned()), "{battlefield:?}");
    assert!(!battlefield.contains(&"Fleshwrither".to_owned()));
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Fleshwrither"]
    );
    Ok(())
}

#[test]
fn transfigure_is_not_offered_on_another_players_turn() -> Result<(), ScriptError> {
    let mut driver = game(2, 2, 1)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    assert_eq!(
        driver.offered_abilities("Fleshwrither")?,
        Vec::<usize>::new()
    );
    Ok(())
}
