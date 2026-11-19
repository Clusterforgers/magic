//! Goal: on the wire a blocked card's reason carries the new details only when they say something: a spell limit names its rule and `limit` (One Voice at two to four seats), a commander's `tax` shows only when it is paid, and every detail reads back as it was sent.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::{DecisionKind, Why};
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

fn mana(cost: &str, tax: u32) -> Why {
    Why::Mana {
        cost: ManaCost::parse(cost).unwrap(),
        at_hand: 1,
        missing: Vec::new(),
        tax,
    }
}

#[rstest]
#[case::no_tax(mana("{1}{G}", 0), false)]
#[case::a_tax(mana("{3}{G}", 2), true)]
fn the_tax_is_sent_only_when_paid(#[case] why: Why, #[case] sent: bool) {
    let wire = serde_json::to_value(&why).unwrap_or(Value::Null);
    assert_eq!(wire["Mana"].get("tax").is_some(), sent);
    assert_eq!(serde_json::from_value::<Why>(wire).ok(), Some(why));
}

#[rstest]
#[case::a_limit(Some(1), json!({"ForbiddenBy": {"source": 4, "name": "One Voice", "limit": 1}}))]
#[case::no_limit(None, json!({"ForbiddenBy": {"source": 4, "name": "One Voice"}}))]
fn a_limit_is_sent_only_with_a_spell_limit(#[case] limit: Option<u32>, #[case] wire: Value) {
    let why = Why::ForbiddenBy {
        source: Some(mtg_core::ids::ObjectId(4)),
        name: Some("One Voice".to_owned()),
        limit,
    };
    assert_eq!(serde_json::to_value(&why).ok(), Some(wire.clone()));
    assert_eq!(serde_json::from_value::<Why>(wire).ok(), Some(why));
}

#[rstest]
fn one_voice_reaches_the_holder_with_its_limit(#[values(2, 3, 4)] players: usize) {
    let rule: Ability = serde_json::from_str(
        r#"{"Rule":{"rule":{"Cant":{"who":"EachPlayer","action":{"Cast":{"more_than":1}}}}}}"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let opt = mtg_testkit::corpus::card("Opt").unwrap_or_else(|error| panic!("{error}"));
    let mut scenario = Scenario::new(players)
        .at(2, 0, Step::Main1)
        .emblems(
            players - 1,
            [Arc::new(emblem_definition("One Voice", &[rule]))],
        )
        .hand(0, [Arc::clone(&opt), opt])
        .battlefield(0, [ready(island()), ready(island())]);
    for seat in 0..players {
        scenario = scenario.library(seat, (0..5).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap_or_else(|error| panic!("{error}"));
    driver.cast("Opt").unwrap_or_else(|error| panic!("{error}"));
    let decision = driver.decision().unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(decision.player, PlayerId(0));
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("a priority decision");
    };
    let held = driver.state().zone(Zone::Hand(PlayerId(0)))[0];
    let wire = serde_json::to_value(options.as_ref()).unwrap_or(Value::Null);
    let entry = wire["unplayable"]
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["card"] == json!(held.0)))
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(entry["reason"], json!("Restricted"));
    assert_eq!(entry["why"]["ForbiddenBy"]["name"], json!("One Voice"));
    assert_eq!(entry["why"]["ForbiddenBy"]["limit"], json!(1));
}
