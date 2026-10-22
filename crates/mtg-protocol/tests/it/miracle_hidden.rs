//! Goal: a miracle card drawn first in a turn is named to every seat by its trigger on the stack and nothing else of the hand is, while the same card drawn second (no trigger) stays unnamed in every other seat's events, table and objects, at two, three and four seats.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameState;
use mtg_engine::zones::Zone;
use mtg_protocol::project_events;
use mtg_testkit::cards::{forest, island, mountain, study};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_in};

const MIRACLE: &str = "Thunderous Wrath";

/// Seat 0 holds a Study and two cards that stay hidden and draws the cards `library` names, then Forests; every other
/// seat has a hand of its own, of other names.
fn table(seats: usize, library: &[&str]) -> Scenario {
    let mut cards: Vec<_> = library
        .iter()
        .map(|name| mtg_testkit::corpus::card(name).unwrap())
        .collect();
    cards.extend(vec![forest(); 6]);
    let mut scenario = Scenario::new(seats)
        .library(0, cards)
        .battlefield(0, vec![ready(island()), ready(mountain())]);
    scenario = scenario.hand(0, {
        let mut hand = vec![study("Study")];
        hand.extend(["Counterspell", "Wrath of God"].map(|n| mtg_testkit::corpus::card(n).unwrap()));
        hand
    });
    let others = [
        ["Serra Angel", "Doom Blade"],
        ["Fireball", "Giant Growth"],
        ["Opt", "Brainstorm"],
    ];
    for seat in 1..seats {
        scenario = scenario
            .hand_named(seat, others[seat - 1])
            .library(seat, vec![forest(); 6]);
    }
    scenario
}

/// Seat 0 casts Study; the stack holds what its draw triggered when this returns.
fn drawn(seats: usize, library: &[&str]) -> (GameState, Driver) {
    let scenario = table(seats, library);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Study").unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    (before, driver)
}

/// The id of the card called `name` in seat 0's hand.
fn in_hand(state: &GameState, name: &str) -> ObjectId {
    state
        .zone(Zone::Hand(PlayerId::from_index(0)))
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .expect("the card is in the hand")
}

#[rstest]
fn the_card_of_a_miracle_trigger_is_named_to_every_seat_and_the_rest_of_the_hand_is_not(
    #[values(2, 3, 4)] seats: usize,
) {
    let (before, driver) = drawn(seats, &[MIRACLE, "Lightning Bolt"]);
    let after = driver.state().clone();
    assert_eq!(after.zone(Zone::Stack).len(), 1, "the trigger is waiting");
    let card = in_hand(&after, MIRACLE);
    let events = driver.engine().events().to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let mut hidden: BTreeMap<ObjectId, String> = hidden_in(&after, *viewer);
        // The trigger names the card it reveals; nothing else of the hand is named.
        hidden.remove(&card);
        assert_eq!(
            hidden.values().any(|name| name == "Wrath of God"),
            viewer.index() != 0,
            "only the owner sees the rest of the hand: {hidden:?}"
        );
        assert_clean(batch, &hidden);
        assert!(
            serde_json::to_string(&batch.objects)
                .unwrap()
                .contains(MIRACLE),
            "seat {} sees the trigger name the card",
            viewer.index()
        );
    }
}

#[rstest]
fn a_card_drawn_second_is_unnamed_in_every_other_seat(#[values(2, 3, 4)] seats: usize) {
    let (before, driver) = drawn(seats, &["Lightning Bolt", MIRACLE]);
    let after = driver.state().clone();
    assert!(after.zone(Zone::Stack).is_empty(), "no trigger");
    let events = driver.engine().events().to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let hidden = hidden_in(&after, *viewer);
        assert_eq!(
            hidden.values().any(|name| name == MIRACLE),
            viewer.index() != 0,
            "the card is in the owner's hand: {hidden:?}"
        );
        assert_clean(batch, &hidden);
    }
}
