//! Goal: "if you lost life last turn" and "if an opponent lost life last turn" (`TalliedLastTurn` of `LifeLost`) read the turn that just ended at two to four seats, whoever lost life in it: the upkeep after the loss triggers for the player the fact names and for no other, and the upkeep after a turn without a loss does not; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, forest, mountain};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::common::pid;

/// An enchantment: "At the beginning of each upkeep, if <who> lost life last turn, you gain 5 life."
fn response(who: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep"}}}},"if":{{"Compare":{{"left":{{"TalliedLastTurn":{{"who":"{who}","what":"LifeLost"}}}},"op":"Ge","right":1}}}},"effects":[{{"GainLife":{{"amount":5}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Response")
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    )
}

#[rstest]
fn ep28_a_loss_counts_in_the_next_turn_only(
    #[values(2, 3, 4)] seats: usize,
    #[values("You", "Opponents")] who: &str,
    #[values(None, Some(0), Some(1))] lost: Option<usize>,
) {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                PresetPermanent::new(response(who)),
                PresetPermanent::new(mountain()),
            ],
        )
        .hand(0, [burn("Bolt", 3)]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    if let Some(seat) = lost {
        let victim = driver.target_player(seat);
        driver.cast_at("Bolt", &[victim]).unwrap();
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
    }
    let life = |driver: &Driver| driver.state().player(pid(0)).unwrap().life();
    let before = life(&driver);
    driver.idle_until(2, Step::Draw).unwrap();
    let fires = match (who, lost) {
        ("You", Some(0)) => true,
        ("Opponents", Some(seat)) => seat != 0,
        _ => false,
    };
    assert_eq!(
        life(&driver) - before,
        if fires { 5 } else { 0 },
        "the next upkeep"
    );
    let after = life(&driver);
    driver.idle_until(3, Step::Draw).unwrap();
    assert_eq!(life(&driver), after, "a turn without a loss");
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
