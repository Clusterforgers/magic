//! Goal: Rowan's Talent copies each loyalty ability activated of the planeswalker it enchants, and of no other, at tables
//! of two to four seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::island;
use mtg_testkit::{Driver, Scenario, attached_to, corpus, ready, with_counters};
use rstest::rstest;

/// A planeswalker with "[+1]: You gain 1 life."
fn walker(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Planeswalker)
            .with_loyalty(3)
            .with_rules(
                serde_json::from_str(
                    r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

#[rstest]
fn the_talent_copies_the_enchanted_planeswalkers_loyalty_abilities(
    #[values(2, 3, 4)] seats: usize,
    #[values("Enchanted", "Other")] activated: &str,
) {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            with_counters(ready(walker("Enchanted")), CounterKind::Loyalty, 3),
            attached_to(ready(corpus::card("Rowan's Talent").unwrap()), 0, 0),
            with_counters(ready(walker("Other")), CounterKind::Loyalty, 3),
        ],
    );
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..10).map(|_| island()))
    });
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    driver.begin_ability(activated, 0).unwrap();
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
    let gained = driver.state().player(PlayerId(0)).unwrap().life() - life;
    assert_eq!(gained, if activated == "Enchanted" { 2 } else { 1 });
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
