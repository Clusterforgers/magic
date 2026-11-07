//! Goal: in a game of two to four seats, a source of "two mana of different colors" pays a spell of two different
//! colors but never two mana of one color, and it makes its two mana with one tap; a mana ability whose cost puts a
//! counter on its source or exerts it pays that cost with the mana, and one limited to once each turn or each game is
//! not offered again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::effect::Produce;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::Event;
use mtg_testkit::cards::instant;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact with "{T}: Add two mana of different colors."
fn vessel() -> Arc<CardDefinition> {
    let mana = serde_json::to_string(&Produce::two_different_colors()).unwrap();
    let rules = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{mana}}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Vessel")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(&rules).unwrap()),
    )
}

#[rstest]
#[case::blue_and_red(2, &[Color::Blue, Color::Red], true)]
#[case::white_and_green(4, &[Color::White, Color::Green], true)]
#[case::two_red(3, &[Color::Red, Color::Red], false)]
#[case::two_green(2, &[Color::Green, Color::Green], false)]
fn two_mana_of_different_colors_pays_only_two_colors(
    #[case] seats: usize,
    #[case] cost: &[Color],
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let table = Scenario::new(seats)
        .hand(0, [instant("Spell", 0, cost)])
        .battlefield(0, [ready(vessel())]);
    let mut driver = Driver::from_scenario(table)?;
    let cast = driver.cast("Spell");
    assert_eq!(cast.is_ok(), offered, "{cast:?}");
    if let Ok(events) = cast {
        let added = events
            .iter()
            .filter(|event| matches!(event, Event::ManaAdded { .. }))
            .count();
        let tapped = events
            .iter()
            .filter(|event| matches!(event, Event::PermanentTapped { .. }))
            .count();
        assert_eq!((added, tapped), (2, 1), "{events:?}");
    }
    Ok(())
}

/// A creature with toughness `toughness` and the mana ability `json`.
fn mana_creature(name: &str, toughness: i32, json: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(0, toughness)
            .with_rules(serde_json::from_str(json).unwrap()),
    )
}

const ROOTS: &str = r#"{"Activated":{"cost":[{"PutCounters":{"counter":{"Other":"-0/-1"}}}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}],"limit":"OncePerTurn"}}"#;
const EXERT: &str =
    r#"{"Activated":{"cost":["Tap","Exert"],"effects":[{"AddMana":{"mana":{"AnyColor":2}}}]}}"#;
const ONCE_A_GAME: &str = r#"{"Activated":{"cost":[{"PayLife":1}],"effects":[{"AddMana":{"mana":{"AnyColor":3}}}],"limit":"OncePerGame"}}"#;

fn counts(events: &[Event]) -> (usize, usize, usize) {
    let count = |hit: fn(&Event) -> bool| events.iter().filter(|event| hit(event)).count();
    (
        count(|event| matches!(event, Event::ManaAdded { .. })),
        count(|event| matches!(event, Event::CountersChanged(_))),
        count(|event| matches!(event, Event::Exerted { .. })),
    )
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn a_counter_put_on_the_source_pays_once_each_turn_even_while_it_is_sick(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let table =
        Scenario::new(seats).battlefield(0, [mtg_testkit::sick(mana_creature("Roots", 5, ROOTS))]);
    let mut driver = Driver::from_scenario(table)?;
    let events = driver.tap_for_mana("Roots")?;
    assert_eq!(counts(&events), (1, 1, 0), "{events:?}");
    assert!(driver.tap_for_mana("Roots").is_err(), "once each turn");
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
fn exerting_the_source_is_part_of_its_mana_cost(#[case] seats: usize) -> Result<(), ScriptError> {
    let table = Scenario::new(seats).battlefield(0, [ready(mana_creature("Ritualist", 1, EXERT))]);
    let mut driver = Driver::from_scenario(table)?;
    let events = driver.tap_for_mana("Ritualist")?;
    assert_eq!(counts(&events), (2, 0, 1), "{events:?}");
    Ok(())
}

#[test]
fn a_mana_ability_limited_to_once_each_game_is_not_offered_again() -> Result<(), ScriptError> {
    let table = Scenario::new(2).battlefield(0, [ready(mana_creature("Exhaust", 1, ONCE_A_GAME))]);
    let mut driver = Driver::from_scenario(table)?;
    let events = driver.tap_for_mana("Exhaust")?;
    assert_eq!(counts(&events).0, 3, "{events:?}");
    assert!(driver.tap_for_mana("Exhaust").is_err(), "once each game");
    Ok(())
}
