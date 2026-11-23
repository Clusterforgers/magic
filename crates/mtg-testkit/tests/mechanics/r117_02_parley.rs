//! Goal: "each player reveals the top card of their library, then draws a card" binds every card revealed this way, so the greatest mana value among them (`Amount::R117GreatestOf`, judged as each card was revealed) and the count of nonland ones read the revealed cards after they were drawn; "a random opponent" takes the damage, never the controller; at two to four seats, and each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, forest, giant};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const REVEAL: &str = r#"{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"RevealUntil":{"who":{"Bound":"player"},"filter":[],"as":"card","revealed_as":"seen"}},{"Bind":{"what":{"Bound":"card"},"as":"revealed","adds":true}},{"Draw":{"who":{"Bound":"player"}}}]}}"#;

fn fool(then: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{REVEAL},{then}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Fool")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

/// Seat 0 reveals a Grizzly Bears (2), seat 1 a Forest (0), seat 2 a Craw Wurm (5), seat 3 a Forest.
fn table(seats: usize, then: &str) -> Driver {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, vec![ready(fool(then))]);
    for seat in 0..seats {
        let card = match seat {
            0 => bear(),
            2 => giant(),
            _ => forest(),
        };
        scenario = scenario.library(seat, (0..5).map(|_| card.clone()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Fool").unwrap();
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            driver.pass().unwrap();
        } else {
            driver.answer_passively().unwrap();
        }
    }
    driver
}

fn hands(driver: &Driver, seats: usize) -> Vec<usize> {
    (0..seats)
        .map(|seat| driver.state().zone(Zone::Hand(PlayerId(seat as u64))).len())
        .collect()
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::two_seats(2, 2)]
#[case::three_seats(3, 5)]
#[case::four_seats(4, 5)]
fn a_random_opponent_takes_the_greatest_mana_value_revealed(
    #[case] seats: usize,
    #[case] greatest: i32,
) {
    let driver = table(
        seats,
        r#"{"ChooseAtRandom":{"from":"Opponents","as":"foe"}},{"DealDamage":{"to":{"Bound":"foe"},"amount":{"R117GreatestOf":{"of":{"Bound":"revealed"},"stat":"ManaValue"}}}}"#,
    );
    assert_eq!(
        hands(&driver, seats),
        vec![1; seats],
        "each player drew the card"
    );
    let lost: Vec<i32> = (0..seats)
        .map(|seat| 20 - driver.state().player(PlayerId(seat as u64)).unwrap().life())
        .collect();
    assert_eq!(lost[0], 0, "the controller is no opponent");
    assert_eq!(lost.iter().sum::<i32>(), greatest, "{lost:?}");
    assert_eq!(lost.iter().filter(|l| **l > 0).count(), 1, "one opponent");
    assert_replays(&driver);
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 2)]
fn each_nonland_card_revealed_this_way_counts(#[case] seats: usize, #[case] nonland: u32) {
    let driver = table(
        seats,
        r#"{"PutCounters":{"on":"This","counter":"+1/+1","count":{"CountOf":{"of":{"Bound":"revealed"},"filter":[{"Not":{"Type":"Land"}}]}}}}"#,
    );
    let fool = driver.id_of("Fool").unwrap();
    let counters = driver
        .state()
        .object(fool)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne);
    assert_eq!(counters, nonland);
    assert_eq!(hands(&driver, seats), vec![1; seats]);
    assert_replays(&driver);
}
