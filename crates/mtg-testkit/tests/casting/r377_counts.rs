//! Goal: "draws 2ˣ cards" (Mathemagics) draws two to the power of the announced X for the target player, one card at X 0,
//! and "your devotion to each color and each combination of colors is increased by one" (Altar of the Pantheon) adds one
//! per Altar of its controller to their devotion to one color or several, none to an opponent's, at two to four seats;
//! each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The log of the driver replays to its state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Lets the stack resolve.
fn settle(driver: &mut Driver) {
    while !driver.state().stack().is_empty() || driver.state().resolving().is_some() {
        driver.pass().unwrap();
    }
}

#[rstest]
fn r377_mathemagics_draws_two_to_the_x(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] x: u32,
    #[values(0, 1)] target: usize,
) {
    let library: Vec<_> = (0..12)
        .map(|i| free_creature(&format!("Card {i}"), 1, 1))
        .collect();
    let lands: Vec<PresetPermanent> = (0..8).map(|_| ready(island())).collect();
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, lands)
        .library(target, library)
        .hand(0, [card("Mathemagics").unwrap()]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(common_pid(target))).len();
    let before = hand(&driver) - usize::from(target == 0);
    driver.begin_cast("Mathemagics").unwrap();
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
        .unwrap();
    let at = driver.target_player(target);
    driver.choose_targets(&[at]).unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    settle(&mut driver);
    assert_eq!(hand(&driver) - before, 1 << x, "X = {x}");
    replays(&scenario, &driver);
}

fn common_pid(seat: usize) -> mtg_core::ids::PlayerId {
    mtg_core::ids::PlayerId::from_index(seat)
}

/// A free sorcery: "You gain life equal to your devotion to `colors`."
fn tithe(colors: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Spell":{{"effects":[{{"GainLife":{{"amount":{{"Devotion":{colors}}}}}}}]}}}}"#
    );
    let rule: Ability = serde_json::from_str(&json).unwrap();
    Arc::new(
        CardDefinition::new("Tithe")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rule),
    )
}

#[rstest]
fn r377_each_altar_adds_one_devotion(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] altars: usize,
    #[values(("[\"Black\"]", 2), ("[\"White\",\"Black\"]", 3), ("[\"Green\"]", 0))] devotion: (
        &str,
        i32,
    ),
) {
    let (colors, printed) = devotion;
    let mut mine = vec![
        ready(creature("Priest", 0, &[Color::Black, Color::Black], 1, 1)),
        ready(creature("Acolyte", 1, &[Color::White], 1, 1)),
    ];
    mine.extend((0..altars).map(|_| ready(card("Altar of the Pantheon").unwrap())));
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, [ready(card("Altar of the Pantheon").unwrap())])
        .hand(0, [tithe(colors)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let life = |driver: &Driver| driver.state().player(common_pid(0)).unwrap().life();
    let before = life(&driver);
    driver.cast("Tithe").unwrap();
    settle(&mut driver);
    let altars = i32::try_from(altars).unwrap();
    assert_eq!(life(&driver) - before, printed + altars, "{colors}");
    replays(&scenario, &driver);
}
