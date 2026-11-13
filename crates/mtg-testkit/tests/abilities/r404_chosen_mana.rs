//! Goal: "Whenever a basic land is tapped for mana of the chosen color, its controller adds an additional one mana of
//! that color" (Gauntlet of Power) adds the chosen color to a tap of a basic land that makes it, and nothing to a tap of
//! a basic land of another color or of a nonbasic land of that color, for whichever color was chosen, at tables of two
//! to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::Supertype;
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::mana::ManaOption;
use mtg_engine::object::Chosen;
use mtg_testkit::cards::{forest, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Gauntlet of Power for no mana.
fn gauntlet() -> Arc<CardDefinition> {
    let mut definition = (*card("Gauntlet of Power").unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

/// A Forest that is not basic, with the Forest's mana ability.
fn nonbasic_forest() -> Arc<CardDefinition> {
    let mut definition = (*forest()).clone();
    "Wild Grove".clone_into(&mut definition.name);
    definition.supertypes.clear();
    Arc::new(definition)
}

fn offers(driver: &Driver) -> BTreeSet<Vec<ManaType>> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options
            .mana_abilities
            .iter()
            .map(ManaOption::made)
            .collect(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// The mana of a color.
fn mana(color: Color) -> ManaType {
    match color {
        Color::White => ManaType::White,
        Color::Blue => ManaType::Blue,
        Color::Black => ManaType::Black,
        Color::Red => ManaType::Red,
        Color::Green => ManaType::Green,
    }
}

/// Seat 0 casts the Gauntlet beside `land`, choosing the `pick`th color offered.
fn table(seats: usize, land: Arc<CardDefinition>, pick: usize) -> Driver {
    let mut scenario = Scenario::new(seats)
        .hand(0, [gauntlet()])
        .battlefield(0, [ready(land)]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| plains()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Gauntlet of Power").unwrap();
    for _ in 0..10 {
        match &driver.decision().unwrap().kind {
            DecisionKind::ChooseOption(_) => {
                driver
                    .answer(Answer::ChooseOption(OptionChoice(pick)))
                    .unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    driver
}

#[rstest]
fn a_basic_land_of_the_chosen_color_adds_one_more(
    #[values(2, 3, 4)] seats: usize,
    #[values((Color::Green, 4), (Color::Blue, 1))] chosen: (Color, usize),
    #[values(0, 1, 2)] which: usize,
) {
    let (color, pick) = chosen;
    let (land, printed) = match which {
        0 => (forest(), ManaType::Green),
        1 => (island(), ManaType::Blue),
        _ => (nonbasic_forest(), ManaType::Green),
    };
    let name = land.name.clone();
    let basic = land.supertypes.contains(&Supertype::Basic);
    let driver = table(seats, land, pick);
    let gauntlet = driver.id_of("Gauntlet of Power").unwrap();
    let held = driver
        .state()
        .object(gauntlet)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    assert_eq!(held, Some(Chosen::Color(color)));
    let mut expected = vec![printed];
    if basic && printed == mana(color) {
        expected.push(printed);
    }
    assert_eq!(
        offers(&driver),
        BTreeSet::from([expected]),
        "{name} with {color:?}"
    );
}
