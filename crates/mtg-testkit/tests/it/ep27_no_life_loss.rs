//! Goal: "You don't lose the game for having 0 or less life" (`CantLoseGame` with `life_only`) keeps its controller in the game at 0 or less life and lets them lose once the permanent is gone, never spares another player (two to four seats) nor a loss to ten poison counters (three and four seats), each log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, mountain};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

const LIFE_ONLY: &str = r#"{"Rule":{"rule":{"CantLoseGame":{"who":"You","life_only":true}}}}"#;

/// A creature with "You don't lose the game for having 0 or less life."
fn unlife() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(LIFE_ONLY).unwrap();
    Arc::new(
        CardDefinition::new("Unlife")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn ep27_a_player_at_zero_life_plays_on_until_the_rule_is_gone(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, -5)] life: i32,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .life(0, life)
        .life(1, if seats > 2 { life } else { 20 })
        .battlefield(
            0,
            [
                PresetPermanent::new(unlife()),
                PresetPermanent::new(mountain()),
            ],
        )
        .hand(0, [burn("Bolt", 1)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    assert!(
        driver.state().is_alive(pid(0)),
        "the rule keeps its controller in"
    );
    assert_eq!(
        driver.state().is_alive(pid(1)),
        seats == 2,
        "another player is not spared"
    );
    let unlife = driver.target_named("Unlife").unwrap();
    driver.cast_at("Bolt", &[unlife]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert!(
        !driver.state().is_alive(pid(0)),
        "the loss happens once the rule is gone"
    );
    replays(&scenario, &driver);
}

#[rstest]
fn ep27_ten_poison_counters_still_lose(#[values(3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .counters(1, CounterKind::Poison, 10)
        .battlefield(1, [PresetPermanent::new(unlife())]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass().unwrap();
    assert!(
        !driver.state().is_alive(pid(1)),
        "only the loss for life is spared"
    );
    replays(&scenario, &driver);
}
