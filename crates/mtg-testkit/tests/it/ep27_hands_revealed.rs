//! Goal: a `RevealHand` rule ("Players play with their hands revealed", "Your opponents ...", "Play with your hand revealed") shows every card of the named hands to every seat and lists them as the revealed hand, a card drawn later included, and shows no other hand; once the permanent is gone the hands are hidden again; at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::sight::revealed_hand;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, creature, mountain};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn telepathy(who: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Rule":{{"rule":{{"RevealHand":"{who}"}}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Seer")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

/// Whether every card of `seat`'s hand is shown to every seat and listed as its revealed hand.
fn shown(driver: &Driver, seat: PlayerId, seats: usize) -> bool {
    let state = driver.state();
    let hand = state.zone(Zone::Hand(seat)).to_vec();
    assert_ne!(hand.len(), 0, "the hand holds cards");
    let all = hand
        .iter()
        .all(|id| (0..seats).all(|viewer| state.is_shown_to(pid(viewer), *id)));
    assert_eq!(all, revealed_hand(state, seat) == hand);
    all
}

#[rstest]
#[case::players("EachPlayer", true, true)]
#[case::opponents("Opponents", false, true)]
#[case::you("You", true, false)]
fn ep27_the_named_hands_are_shown_until_the_rule_is_gone(
    #[case] who: &str,
    #[case] mine: bool,
    #[case] theirs: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                PresetPermanent::new(telepathy(who)),
                PresetPermanent::new(mountain()),
            ],
        )
        .hand(0, [burn("Bolt", 1), creature("Bear", 1, &[], 2, 2)])
        .hand(1, [creature("Elk", 1, &[], 2, 2)])
        .library(1, [creature("Owl", 1, &[], 1, 1)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(shown(&driver, pid(0), seats), mine);
    assert_eq!(shown(&driver, pid(1), seats), theirs);
    let seer = driver.target_named("Seer").unwrap();
    driver.cast_at("Bolt", &[seer]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert!(!shown(&driver, pid(0), seats));
    assert!(!shown(&driver, pid(1), seats));
}

#[test]
fn ep27_a_card_drawn_later_is_shown() {
    let seats = 2;
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [PresetPermanent::new(telepathy("Opponents"))])
        .hand(1, [creature("Elk", 1, &[], 2, 2)])
        .library(
            1,
            [creature("Owl", 1, &[], 1, 1), creature("Fox", 1, &[], 1, 1)],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(4, Step::Main1).unwrap();
    let state = driver.state();
    assert_eq!(state.zone(Zone::Hand(pid(1))).len(), 2, "seat 1 drew");
    assert!(shown(&driver, pid(1), seats));
}
