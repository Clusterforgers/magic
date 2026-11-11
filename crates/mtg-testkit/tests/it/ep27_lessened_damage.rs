//! Goal: a static replacement that lessens damage (`Subtract`), caps it (`Cap`) or halves it rounded down (`Half`) changes the damage a spell deals to a player by that rule across amounts, deals none when it comes to 0 (no life lost), leaves damage to another recipient as it was, and the log replays to the live state; at two and three seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

/// An enchantment with a replacement `by` of damage dealt to `to` (any recipient when empty).
fn ward(by: &str, to: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Replacement":{{"when":{{"Damage":{{{to}}}}},"by":{by}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Ward")
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    )
}

#[rstest]
#[case::minus_one_of_three(r#"{"Subtract":1}"#, 3, 2)]
#[case::minus_one_of_one(r#"{"Subtract":1}"#, 1, 0)]
#[case::minus_two_of_one(r#"{"Subtract":2}"#, 1, 0)]
#[case::cap_three_of_five(r#"{"Cap":3}"#, 5, 3)]
#[case::cap_three_of_two(r#"{"Cap":3}"#, 2, 2)]
#[case::half_of_five(r#""Half""#, 5, 2)]
#[case::half_of_one(r#""Half""#, 1, 0)]
fn ep27_damage_is_lessened(
    #[case] by: &str,
    #[case] dealt: i32,
    #[case] lost: i32,
    #[values(2, 3)] seats: usize,
    #[values(true, false)] to_players: bool,
) {
    let to = if to_players {
        r#""to":"EachPlayer""#
    } else {
        ""
    };
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                PresetPermanent::new(ward(by, to)),
                PresetPermanent::new(mountain()),
            ],
        )
        .hand(0, [burn("Bolt", dealt)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let life = driver.state().player(pid(1)).unwrap().life();
    let foe = driver.target_player(1);
    driver.cast_at("Bolt", &[foe]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(life - driver.state().player(pid(1)).unwrap().life(), lost);
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// A replacement of damage to creatures leaves damage to a player as it was.
#[rstest]
fn ep27_another_recipient_keeps_its_damage(#[values(2, 3)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                PresetPermanent::new(ward(
                    r#"{"Subtract":1}"#,
                    r#""to":{"Each":[{"Type":"Creature"}]}"#,
                )),
                PresetPermanent::new(mountain()),
            ],
        )
        .hand(0, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let life = driver.state().player(pid(1)).unwrap().life();
    let foe = driver.target_player(1);
    driver.cast_at("Bolt", &[foe]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(life - driver.state().player(pid(1)).unwrap().life(), 3);
}
