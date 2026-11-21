//! Goal: "As this creature enters, pay any amount of life" at 2 to 4 seats (CR 119.4, 614.12): Minion of the Wastes and
//! Nameless Race ask for the amount up to the life total, Nameless Race also up to the white nontoken permanents its
//! controller's opponents control plus the white cards in their graveyards; the life is paid before the creature is on
//! the battlefield and its power and toughness are the amount paid; paying nothing makes it a 0/0.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{NumberChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds `name` with six Swamps; seat 1 controls `white` white creatures and holds as many in its graveyard.
fn game(seats: usize, name: &str, white: usize) -> Result<Driver, ScriptError> {
    let white_card = |n: usize| creature(&format!("Knight {n}"), 0, &[Color::White], 2, 2);
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![card(name).unwrap()])
        .battlefield(0, (0..6).map(|_| ready(swamp())))
        .battlefield(1, (0..white).map(|n| ready(white_card(n))))
        .graveyard(1, (0..white).map(|n| white_card(n + 10)));
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 4]);
    }
    Driver::from_scenario(scenario)
}

/// Casts the creature, lets it resolve, answers yes to paying and `paid` for the amount; the most offered.
fn cast_paying(driver: &mut Driver, name: &str, paid: Option<u32>) -> Result<u32, ScriptError> {
    let lands = ["Swamp"; 6];
    let cost = if name == "Nameless Race" { 4 } else { 6 };
    driver.cast_tapping(name, &lands[..cost])?;
    let mut most = 0;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(paid.is_some())))?;
            }
            DecisionKind::ChooseNumber(options) => {
                most = options.max.unwrap_or(0);
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(
                    paid.unwrap_or(0),
                )))?;
            }
            _ if driver.state().stack().is_empty() => return Ok(most),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the creature does not resolve".into()))
}

#[rstest]
#[case::minion_pays_seven("Minion of the Wastes", 0, Some(7), 20)]
#[case::minion_pays_nothing("Minion of the Wastes", 0, None, 20)]
#[case::race_capped_by_white("Nameless Race", 2, Some(3), 4)]
#[case::race_without_white("Nameless Race", 0, Some(0), 0)]
fn r114_life_paid_as_it_enters_is_its_power_and_toughness(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] white: usize,
    #[case] paid: Option<u32>,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, name, white)?;
    let offered = cast_paying(&mut driver, name, paid)?;
    if paid.is_some() {
        assert_eq!(offered, most, "the most life offered");
    }
    let paid = paid.unwrap_or(0);
    assert_eq!(
        driver.state().player(PlayerId(0)).unwrap().life(),
        20 - i32::try_from(paid).unwrap()
    );
    if paid == 0 {
        assert!(
            driver.id_of(name).is_err(),
            "a 0/0 is put into the graveyard"
        );
        assert_eq!(driver.state().zone(Zone::Graveyard(PlayerId(0))).len(), 1);
        return Ok(());
    }
    let id = driver.id_of(name)?;
    let view = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!(
        (view.power, view.toughness),
        (
            Some(i32::try_from(paid).unwrap()),
            Some(i32::try_from(paid).unwrap())
        )
    );
    Ok(())
}

#[rstest]
fn r114_more_life_than_the_cap_is_refused(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, "Nameless Race", 1)?;
    driver.cast_tapping("Nameless Race", &["Swamp"; 4])?;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::ChooseNumber(options) => {
                assert_eq!(options.max, Some(2));
                assert!(
                    driver
                        .answer(Answer::ChooseNumber(NumberChoice::Number(3)))
                        .is_err()
                );
                return Ok(());
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("no amount asked".into()))
}
