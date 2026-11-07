//! Goal: in a game of two to four seats, mana whose rider gives a permanent spell more counters or a keyword (Guildmages'
//! Forum, Generator Servant) makes the creature it pays for enter with the counters and have the keyword, and a spell
//! outside the rider's kind gets neither.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_testkit::cards::creature;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A land that taps for {G} with the rider `bonus`.
fn land(bonus: &str) -> Arc<CardDefinition> {
    let rules = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["Green"]}},"only":[{{"Enters":{bonus}}}]}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Rider Land")
            .with_type(CardType::Land)
            .with_rules(serde_json::from_str(&rules).unwrap()),
    )
}

const COUNTER: &str = r#"{"spell":{"types":["Creature"]},"counters":1}"#;
const MULTICOLORED: &str = r#"{"spell":{"types":["Creature"],"multicolored":true},"counters":1}"#;
const HASTE: &str = r#"{"spell":{"types":["Creature"]},"keyword":"Haste","this_turn":true}"#;
const HASTE_FOR_GOOD: &str = r#"{"spell":{"types":["Creature"]},"keyword":"Haste"}"#;
const DRAGON_HASTE: &str =
    r#"{"spell":{"types":["Creature"],"subtypes":["Dragon"]},"keyword":"Haste","this_turn":true}"#;

/// Casts a one-mana green Bear paid by the land and resolves it.
fn play(seats: usize, bonus: &str) -> Driver {
    let table = Scenario::new(seats)
        .hand(0, [creature("Bear", 0, &[Color::Green], 2, 2)])
        .battlefield(0, [ready(land(bonus))]);
    let mut driver = Driver::from_scenario(table).unwrap();
    driver.cast("Bear").unwrap();
    for _ in 0..12 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass().unwrap();
    }
    driver
}

fn counters(driver: &Driver) -> u32 {
    let id = driver.id_of("Bear").unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .expect("the Bear is on the battlefield")
        .counters()
        .filter(|(kind, _)| **kind == CounterKind::PlusOnePlusOne)
        .map(|(_, n)| n)
        .sum()
}

fn hasty(driver: &Driver) -> bool {
    let id = driver.id_of("Bear").unwrap();
    keywords_of(driver.state(), id).contains(&&Keyword::Haste)
}

#[rstest]
#[case::a_creature_spell(2, COUNTER, 1, false)]
#[case::not_multicolored(3, MULTICOLORED, 0, false)]
#[case::haste_this_turn(4, HASTE, 0, true)]
#[case::haste_for_good(2, HASTE_FOR_GOOD, 0, true)]
#[case::not_a_dragon(3, DRAGON_HASTE, 0, false)]
fn the_creature_the_mana_pays_for_gets_what_the_rider_gives(
    #[case] seats: usize,
    #[case] bonus: &str,
    #[case] expected_counters: u32,
    #[case] haste: bool,
) {
    let driver = play(seats, bonus);
    assert_eq!(counters(&driver), expected_counters);
    assert_eq!(hasty(&driver), haste);
}

#[rstest]
fn haste_until_end_of_turn_ends_with_the_turn(#[values(2, 3)] seats: usize) {
    let mut driver = play(seats, HASTE);
    assert!(hasty(&driver));
    let turn = driver.state().turn().number();
    driver.idle_until(turn + 1, Step::Upkeep).unwrap();
    assert!(!hasty(&driver), "the haste ended with the turn");
}
