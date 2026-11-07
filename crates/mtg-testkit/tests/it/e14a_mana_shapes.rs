//! Goal: in a game of two to four seats, a source of "two mana of different colors" pays a spell of two different
//! colors but never two mana of one color, and it makes its two mana with one tap.
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
