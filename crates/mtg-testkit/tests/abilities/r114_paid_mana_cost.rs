//! Goal: "Exile a creature card from your graveyard and pay its mana cost" at 2 to 4 seats (CR 601.2f, 602.2b): Back
//! from the Brink's activation costs the mana cost of the card picked for it, so a cheap card is paid with one land and
//! leaves the others untapped, a dear one takes more, and the token is a copy of the exiled card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Back from the Brink and four Forests; its graveyard holds a Cub ({G}) and a Beast ({3}{G}).
fn brink(seats: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Back from the Brink").unwrap()),
                ready(forest()),
                ready(forest()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .graveyard(
            0,
            vec![
                creature("Cub", 0, &[Color::Green], 1, 1),
                creature("Beast", 3, &[Color::Green], 4, 4),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 4]);
    }
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::cub("Cub", 3, 1)]
#[case::beast("Beast", 0, 4)]
fn r114_the_activation_costs_the_mana_cost_of_the_card_exiled(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] untapped: usize,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    let mut driver = brink(seats)?;
    driver.begin_ability("Back from the Brink", 0)?;
    let picked = *driver
        .state()
        .zone(Zone::Graveyard(PlayerId(0)))
        .iter()
        .find(|id| driver.state().object(**id).unwrap().card().name == name)
        .unwrap();
    for _ in 0..10 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(_) => {
                driver.answer(Answer::ChooseCards(CardsChoice(vec![picked])))?;
            }
            DecisionKind::PayMana(options) => {
                assert_eq!(options.cost.mana_value(), u32::try_from(power).unwrap());
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    for _ in 0..seats {
        driver.pass()?;
    }
    let forests = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == "Forest")
        .filter(|object| object.permanent().is_some_and(|p| !p.tapped()))
        .count();
    assert_eq!(forests, untapped);
    assert_eq!(driver.state().object(picked).unwrap().zone(), Zone::Exile);
    let token = driver.id_of(name)?;
    let view = mtg_engine::characteristics::characteristics(driver.state(), token).unwrap();
    assert_eq!(view.power, Some(power));
    Ok(())
}
