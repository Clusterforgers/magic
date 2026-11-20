//! Goal: Foggy Swamp Visions exiles X creature cards from any graveyards, creates a token copy of each card it exiled
//! (the copiable values of a card in exile, CR 707.2) under its caster's control, and sacrifices those tokens at the
//! beginning of its caster's next end step; creatures tapped for its "waterbend {X}" pay that X (CR 601.2b, 601.2g).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::{NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{names, pid};

/// The graveyard card called `name` as a target.
fn in_graveyard(driver: &Driver, name: &str) -> Target {
    let object = driver
        .state()
        .objects()
        .find(|object| object.card().name == name && matches!(object.zone(), Zone::Graveyard(_)))
        .unwrap();
    Target::Object(object.reference())
}

fn tokens(driver: &Driver) -> Vec<String> {
    let mut found: Vec<String> = names(driver.state(), Zone::Battlefield)
        .into_iter()
        .filter(|name| name.starts_with("Grave "))
        .collect();
    found.sort();
    found
}

#[rstest]
fn r111_each_exiled_creature_card_becomes_a_token_until_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [card("Foggy Swamp Visions").unwrap()])
            .battlefield(0, lands(5, swamp))
            .graveyard(0, [creature("Grave Bear", 1, &[Color::Green], 2, 2)])
            .graveyard(seats - 1, [creature("Grave Ogre", 2, &[Color::Red], 3, 3)]),
    )?;
    driver.begin_cast("Foggy Swamp Visions")?;
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(2)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let targets = [
                    in_graveyard(&driver, "Grave Bear"),
                    in_graveyard(&driver, "Grave Ogre"),
                ];
                driver.choose_targets(&targets)?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(tokens(&driver), ["Grave Bear", "Grave Ogre"]);
    assert_eq!(names(driver.state(), Zone::Exile).len(), 2);
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(tokens(&driver), Vec::<String>::new());
    assert!(
        names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Foggy Swamp Visions".to_owned())
    );
    Ok(())
}

/// Whether the permanent called `name` is tapped.
fn tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

// Waterbend {X} with X = 2 paid by tapping two creatures: the three Swamps pay {1}{B}{B} alone.
#[rstest]
fn r111_creatures_tapped_for_waterbend_pay_the_x_of_the_exiled_cards(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut board = lands(3, swamp);
    board.extend(["Helper One", "Helper Two"].map(|name| ready(free_creature(name, 1, 1))));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [card("Foggy Swamp Visions").unwrap()])
            .battlefield(0, board)
            .graveyard(0, [creature("Grave Bear", 1, &[Color::Green], 2, 2)])
            .graveyard(seats - 1, [creature("Grave Ogre", 2, &[Color::Red], 3, 3)]),
    )?;
    driver.begin_cast("Foggy Swamp Visions")?;
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(2)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let targets = [
                    in_graveyard(&driver, "Grave Bear"),
                    in_graveyard(&driver, "Grave Ogre"),
                ];
                driver.choose_targets(&targets)?;
            }
            DecisionKind::PayMana(_) => {
                driver.pay_helped_by(
                    &["Swamp", "Swamp", "Swamp"],
                    &[],
                    &["Helper One", "Helper Two"],
                    &[],
                    0,
                )?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert!(tapped(&driver, "Helper One") && tapped(&driver, "Helper Two"));
    assert_eq!(tokens(&driver), ["Grave Bear", "Grave Ogre"]);
    let exiled = names(driver.state(), Zone::Exile);
    assert!(exiled.contains(&"Grave Bear".to_owned()) && exiled.contains(&"Grave Ogre".to_owned()));
    Ok(())
}
