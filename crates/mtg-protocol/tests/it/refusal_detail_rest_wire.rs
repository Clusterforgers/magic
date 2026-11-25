//! Goal: on the wire the last refusals told in detail read back as sent: a least X sends its mana only when the mana is short, a chosen color sends the color and its cost only when one is chosen, and at two to four seats the holder's offer carries the target a spell's every payable X leaves out.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::{DecisionKind, Why};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

#[rstest]
#[case::least_x_short_of_mana(Why::LeastX { least: 1, cost: Some(ManaCost::parse("{1}{G}").unwrap()), at_hand: Some(1) }, &["least", "cost", "at_hand"])]
#[case::least_x_short_of_counters(Why::LeastX { least: 2, cost: None, at_hand: None }, &["least"])]
#[case::a_chosen_color(Why::ChosenColor { color: Some(Color::Red), cost: Some(ManaCost::parse("{R}{R}{R}").unwrap()), at_hand: 1 }, &["color", "cost", "at_hand"])]
#[case::no_chosen_color(Why::ChosenColor { color: None, cost: None, at_hand: 0 }, &["at_hand"])]
fn a_detail_sends_only_what_it_has(#[case] why: Why, #[case] keys: &[&str]) {
    let wire = serde_json::to_value(&why).unwrap_or(Value::Null);
    let body = wire
        .as_object()
        .and_then(|outer| outer.values().next())
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut sent: Vec<&str> = body.keys().map(String::as_str).collect();
    sent.sort_unstable();
    let mut wanted = keys.to_vec();
    wanted.sort_unstable();
    assert_eq!(sent, wanted);
    assert_eq!(serde_json::from_value::<Why>(wire).ok(), Some(why));
}

#[rstest]
fn the_holder_reads_the_target_no_payable_x_leaves(#[values(2, 3, 4)] players: usize) {
    let melody = mtg_testkit::corpus::card("Entrancing Melody").unwrap();
    let angel = mtg_testkit::corpus::card("Serra Angel").unwrap();
    let mut scenario = Scenario::new(players)
        .at(2, 0, Step::Main1)
        .hand(0, [melody])
        .battlefield(0, (0..4).map(|_| ready(island())))
        .battlefield(1, [ready(angel)]);
    for seat in 0..players {
        scenario = scenario.library(seat, (0..5).map(|_| island()));
    }
    let driver = Driver::from_scenario(scenario).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("a priority decision");
    };
    let card = driver.state().zone(Zone::Hand(mtg_core::ids::PlayerId(0)))[0];
    let wire = serde_json::to_value(options.as_ref()).unwrap_or(Value::Null);
    let entry = wire["unplayable"]
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["card"] == json!(card.0)))
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(entry["reason"], json!("Targets"));
    assert_eq!(
        entry["why"],
        json!({"XTarget": {"needs": "a creature with mana value X on the battlefield", "zone": "Battlefield", "least": 0, "most": 2}})
    );
}
