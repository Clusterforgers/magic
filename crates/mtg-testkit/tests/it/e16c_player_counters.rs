//! Goal: "whenever you get one or more {E}" triggers once for the counters one effect gives its controller, however many, and not for counters another player gets or counters of another kind, at tables of two, three and four seats; "an opponent gets" triggers once for each opponent who gets them, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::cost;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free enchantment with these rules.
fn e16c_enchantment(name: &str, rules: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// "At the beginning of your upkeep, `who` gets two `kind` counters."
fn e16c_giver(who: &str, kind: &str) -> Arc<CardDefinition> {
    e16c_enchantment(
        "Giver",
        &format!(
            r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"effects":[{{"PutCounters":{{"on":"{who}","counter":{kind},"count":2}}}}]}}}}"#
        ),
    )
}

/// "Whenever `who` gets one or more {E}, you gain 1 life."
fn e16c_watcher(who: &str) -> Arc<CardDefinition> {
    e16c_enchantment(
        "Watcher",
        &format!(
            r#"{{"Triggered":{{"when":{{"Batch":{{"PlayerCountersPut":{{"who":"{who}","counter":{{"Other":"energy"}}}}}}}},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
        ),
    )
}

const ENERGY: &str = r#"{"Other":"energy"}"#;
const POISON: &str = r#""poison""#;

/// Seat 0 has the Giver and its upkeep begins; the Watcher is on seat `watcher`'s battlefield and its controller gains
/// `gained` life.
#[rstest]
#[case::you_get_energy(2, "You", ENERGY, 0, "You", 1)]
#[case::three_seats(3, "You", ENERGY, 0, "You", 1)]
#[case::another_player_gets_it(3, "You", ENERGY, 1, "You", 0)]
#[case::another_kind(3, "You", POISON, 0, "You", 0)]
#[case::an_opponent_gets_it(3, "You", ENERGY, 1, "Opponents", 1)]
#[case::each_opponent_gets_it(4, "Opponents", ENERGY, 0, "Opponents", 3)]
#[case::each_player_gets_it(4, "EachPlayer", ENERGY, 2, "You", 1)]
fn e16c_getting_counters_triggers_once_per_player(
    #[case] seats: usize,
    #[case] given: &str,
    #[case] kind: &str,
    #[case] watcher: usize,
    #[case] watched: &str,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let scenario = || {
        let giver = ready(e16c_giver(given, kind));
        let watching = ready(e16c_watcher(watched));
        let scenario = Scenario::new(seats).at(3, seats - 1, Step::End);
        if watcher == 0 {
            scenario.battlefield(0, vec![giver, watching])
        } else {
            scenario
                .battlefield(0, vec![giver])
                .battlefield(watcher, vec![watching])
        }
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before = driver.state().player(pid(watcher)).unwrap().life();
    driver.pass_until(Step::Upkeep)?;
    for _ in 0..40 {
        if driver.state().turn().step() != Step::Upkeep {
            break;
        }
        driver.answer_passively()?;
    }
    let after = driver.state().player(pid(watcher)).unwrap().life();
    assert_eq!(after - before, gained);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
