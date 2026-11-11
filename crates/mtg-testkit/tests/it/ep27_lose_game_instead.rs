//! Goal: "If you would lose the game, instead exile ~ and your life total becomes 1." (a `LoseGame` replacement by `Instead`) keeps its controller in at 0 or less life and with ten poison counters, exiles the permanent and sets the life to 1, works once (the next loss happens), never covers another player, and each log replays to the live state; at three and four seats.
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
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

const THRONE: &str = r#"{"Replacement":{"when":{"LoseGame":{}},"by":{"Instead":[{"Exile":{"what":"This"}},{"SetLife":{"amount":1}}]}}}"#;

fn throne() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(THRONE).unwrap();
    Arc::new(
        CardDefinition::new("Throne")
            .with_type(CardType::Artifact)
            .with_rules(rule),
    )
}

fn named_in(driver: &Driver, zone: Zone) -> bool {
    driver.state().zone(zone).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == "Throne")
    })
}

fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn ep27_the_loss_is_replaced_once(
    #[values(3, 4)] seats: usize,
    #[values(true, false)] poison: bool,
) {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(1, [PresetPermanent::new(throne())])
        .life(2, if poison && seats == 3 { 20 } else { 0 });
    scenario = if poison {
        scenario.counters(1, CounterKind::Poison, 10)
    } else {
        scenario.life(1, 0)
    };
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass().unwrap();
    let state = driver.state();
    assert_eq!(
        state.is_alive(pid(2)),
        poison && seats == 3,
        "another player is not covered"
    );
    if poison {
        // The poison counters remain, so the next check is a loss with the throne gone.
        assert!(!state.is_alive(pid(1)), "the replacement works once");
    } else {
        assert!(state.is_alive(pid(1)), "the loss is replaced");
        assert_eq!(state.player(pid(1)).unwrap().life(), 1);
        assert!(named_in(&driver, Zone::Exile), "the throne is exiled");
        assert!(!named_in(&driver, Zone::Battlefield));
    }
    replays(&scenario, &driver);
}
