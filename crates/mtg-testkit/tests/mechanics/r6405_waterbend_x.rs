//! Goal: an ability whose cost is "Waterbend {X}. X can't be 0." announces X from 1 up to what lands, artifacts and creatures (the source included when it does not tap) can pay, taps exactly X helpers, and sets the base power and toughness of the creatures it names to the X announced; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const BEND: &str = r#"{"Activated":{"cost":[{"Waterbend":"X"},{"XAtLeast":1}],"condition":"YourTurn","effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"SetPT":{"power":"X","toughness":"X"}}],"duration":"UntilEndOfTurn"}}]}}"#;

/// A 1/1 creature with the ability.
fn waterbender() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(BEND).unwrap();
    Arc::new(
        CardDefinition::new("Waterbender")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

fn relic(n: usize) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(format!("Relic {n}")).with_type(CardType::Artifact))
}

/// Seat 0 (on turn) has the Waterbender, `lands` Forests, `relics` artifacts and `bears` 2/2 creatures.
fn game(seats: usize, lands: usize, relics: usize, bears: usize) -> Result<Driver, ScriptError> {
    let mut board = vec![ready(waterbender())];
    board.extend((0..lands).map(|_| ready(forest())));
    board.extend((1..=relics).map(|n| ready(relic(n))));
    board.extend((1..=bears).map(|n| ready(free_creature(&format!("Bear {n}"), 2, 2))));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .battlefield(0, board)
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![forest(); 5]),
    );
    Driver::from_scenario(scenario)
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

#[rstest]
#[case::lands_only(3, 0, 0, Some((1, 4)))]
#[case::helpers_count(0, 2, 1, Some((1, 4)))]
#[case::the_source_helps_when_it_does_not_tap(0, 0, 0, None)]
fn x_runs_from_1_to_what_lands_and_helpers_reach(
    #[case] lands: usize,
    #[case] relics: usize,
    #[case] bears: usize,
    #[case] asked: Option<(u32, u32)>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, lands, relics, bears)?;
    driver.begin_activation("Waterbender")?;
    match &driver.decision()?.kind {
        DecisionKind::ChooseNumber(options) => {
            assert_eq!(Some((options.min, options.max.unwrap())), asked);
        }
        DecisionKind::PayMana(_) => assert_eq!(asked, None, "X = 1 is taken without asking"),
        other => panic!("{}", other.name()),
    }
    Ok(())
}

#[rstest]
fn the_creatures_become_x_by_x_and_x_helpers_tap(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 1, 1)?;
    driver.begin_activation("Waterbender")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(3)))?;
    driver.pay_helped_by(&["Forest"], &[], &["Relic 1", "Bear 1"], &[], 0)?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    for name in ["Waterbender", "Bear 1"] {
        assert_eq!(stats(&driver, name), (3, 3), "{name}");
    }
    Ok(())
}

#[test]
fn the_automatic_payment_pays_x() -> Result<(), ScriptError> {
    let mut driver = game(2, 0, 2, 0)?;
    driver.begin_activation("Waterbender")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(2)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert_eq!(stats(&driver, "Waterbender"), (2, 2));
    Ok(())
}
