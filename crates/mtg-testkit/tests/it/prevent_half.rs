//! Goal: "prevent half that damage, rounded up" (Gisela, Blade of Goldnight) takes the larger half of each damage event to its controller and leaves damage to others whole, at tables of two to four seats, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::replay;
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// An enchantment: "If a source would deal damage to you, prevent half that damage, rounded up."
fn halver() -> Arc<CardDefinition> {
    let rule = r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":"PreventHalf"}}"#;
    Arc::new(
        CardDefinition::new("Halver")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(rule).unwrap()),
    )
}

#[rstest]
#[case::three_is_two_prevented("Lightning Bolt", 0, 1)]
#[case::burst_for_two_is_one_prevented("Burst Lightning", 0, 1)]
#[case::an_opponent_takes_it_whole("Lightning Bolt", 1, 3)]
fn the_larger_half_of_the_damage_to_the_controller_is_prevented(
    #[values(2, 3, 4)] seats: usize,
    #[case] burn: &str,
    #[case] victim: usize,
    #[case] dealt: i32,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [card(burn).unwrap()])
        .battlefield(0, [ready(land(Color::Red)), ready(halver())]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().player(pid(victim)).unwrap().life();
    let target = driver.target_player(victim);
    driver.cast_at(burn, &[target])?;
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(
        before - driver.state().player(pid(victim)).unwrap().life(),
        dealt
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
