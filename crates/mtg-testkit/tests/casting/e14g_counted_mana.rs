//! Goal: in a game of two to four seats, "for each color among permanents you control, add one mana of that color"
//! (Bloom Tender) adds one mana of each colour its controller's permanents have, and "choose a color, add mana equal to
//! your devotion to it" (Nykthos) offers each colour of some devotion and adds that many; a colour of no devotion is not
//! offered.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_engine::Event;
use mtg_testkit::cards::creature;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A colourless artifact that taps for `mana`.
fn source(name: &str, mana: &str) -> Arc<CardDefinition> {
    let rules = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{mana}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(&rules).unwrap()),
    )
}

const EACH: &str = r#"{"EachColorAmong":[{"ControlledBy":"You"}]}"#;

/// The mana the events add, in order.
fn added(events: &[Event]) -> Vec<ManaType> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { mana, .. } => Some(*mana),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::colourless_alone(2, vec![], &[])]
#[case::one_green(3, vec![vec![Color::Green]], &[ManaType::Green])]
#[case::two_colours_once_each(4, vec![vec![Color::Green], vec![Color::Blue, Color::Green]], &[ManaType::Blue, ManaType::Green])]
#[case::three_colours(2, vec![vec![Color::White, Color::Red], vec![Color::Black]], &[ManaType::White, ManaType::Black, ManaType::Red])]
fn each_colour_among_your_permanents_adds_one_mana(
    #[case] seats: usize,
    #[case] creatures: Vec<Vec<Color>>,
    #[case] expected: &[ManaType],
) {
    let mut battlefield = vec![ready(source("Tender", EACH))];
    battlefield.extend(
        creatures
            .iter()
            .enumerate()
            .map(|(at, colors)| ready(creature(&format!("Bear {at}"), 0, colors, 2, 2))),
    );
    let table = Scenario::new(seats)
        .battlefield(0, battlefield)
        .battlefield(1, [ready(creature("Foe", 0, &[Color::Red], 2, 2))]);
    let mut driver = Driver::from_scenario(table).unwrap();
    match driver.tap_for_mana("Tender") {
        Ok(events) => {
            let mut mana = added(&events);
            mana.sort_by_key(|kind| kind.symbol());
            let mut wanted = expected.to_vec();
            wanted.sort_by_key(|kind| kind.symbol());
            assert_eq!(mana, wanted);
        }
        Err(_) => assert!(expected.is_empty(), "no colour adds nothing to offer"),
    }
}

#[rstest]
#[case::two_green(2, ManaType::Green, 2)]
#[case::one_blue(3, ManaType::Blue, 1)]
#[case::no_red(4, ManaType::Red, 0)]
fn devotion_adds_as_many_mana_of_the_colour_picked(
    #[case] seats: usize,
    #[case] colour: ManaType,
    #[case] expected: usize,
) {
    let table = Scenario::new(seats).battlefield(
        0,
        [
            ready(source("Shrine", r#""Devotion""#)),
            ready(creature("Elf", 0, &[Color::Green, Color::Green], 1, 1)),
            ready(creature("Merfolk", 1, &[Color::Blue], 1, 1)),
        ],
    );
    let mut driver = Driver::from_scenario(table).unwrap();
    match driver.tap_for_mana_of("Shrine", colour) {
        Ok(events) => assert_eq!(added(&events), vec![colour; expected]),
        Err(_) => assert_eq!(expected, 0, "a colour of no devotion is not offered"),
    }
}
