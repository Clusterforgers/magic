//! Goal: a card suspended from a hand, and a madness card discarded into exile, are open to every seat as they leave the hand (the move, the card on the table and the time counters it holds), while the rest of the hand stays unnamed in every seat's events, table and objects, the owner's other seats included.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, PayChoice, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, project_events};
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_in};

/// Seat 0 holds the card `open` and two cards that stay hidden; every other seat has a hand of its own, of other names.
fn table(seats: usize, open: &str, battlefield: Vec<mtg_engine::PresetPermanent>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 6])
        .hand_named(0, [open, "Counterspell", "Wrath of God"])
        .battlefield(0, battlefield);
    let others = [
        ["Serra Angel", "Doom Blade"],
        ["Fireball", "Giant Growth"],
        ["Opt", "Brainstorm"],
    ];
    for seat in 1..seats {
        scenario = scenario.hand_named(seat, others[seat - 1]);
    }
    scenario
}

/// Checks the events of `driver` since `before` from every seat: the card `open` is named and moved to exile in the open,
/// and nothing that is hidden is named.
fn assert_open(seats: usize, before: &GameState, driver: &Driver, open: &str) {
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let card = after
        .zone(Zone::Exile)
        .iter()
        .copied()
        .find(|id| after.object(*id).is_some_and(|o| o.card().name == open))
        .expect("the card is in exile");
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let hidden: BTreeMap<ObjectId, String> = hidden_in(&after, *viewer);
        assert!(!hidden.contains_key(&card), "the exiled card is not hidden");
        assert_eq!(
            hidden.values().any(|name| name == "Wrath of God"),
            viewer.index() != 0,
            "only the owner sees the rest of the hand: {hidden:?}"
        );
        assert_clean(batch, &hidden);
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                EventView::Public(Event::ZoneChanged { object, to: Zone::Exile, .. }) if *object == card
            )),
            "seat {} sees the card go to exile",
            viewer.index()
        );
        assert!(
            serde_json::to_string(&batch.objects)
                .unwrap()
                .contains(open),
            "seat {} sees the card in exile",
            viewer.index()
        );
    }
}

#[rstest]
fn a_suspended_card_is_open_to_every_seat_and_the_rest_of_the_hand_is_not(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = table(seats, "Rift Bolt", vec![ready(mountain())]);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let bolt = driver.state().zone(Zone::Hand(PlayerId::from_index(0)))[0];
    driver
        .answer(Answer::Priority(PriorityAction::Suspend(bolt)))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    assert_open(seats, &before, &driver, "Rift Bolt");
}

#[rstest]
fn a_madness_card_discarded_into_exile_is_open_to_every_seat_and_the_rest_of_the_hand_is_not(
    #[values(2, 3, 4)] seats: usize,
) {
    let goblin = mtg_testkit::corpus::card("Rummaging Goblin").unwrap();
    let scenario = table(seats, "Fiery Temper", vec![ready(goblin)]);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Rummaging Goblin").unwrap();
    // The cost discards the card the player picks: Fiery Temper, and the rest stay.
    let temper = driver.state().zone(Zone::Hand(PlayerId::from_index(0)))[0];
    driver
        .answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
            temper,
        ])))
        .unwrap();
    let batch = driver.engine().events().to_vec();
    assert!(
        batch.iter().any(|event| matches!(
            event,
            Event::ZoneChanged {
                to: Zone::Exile,
                ..
            }
        )),
        "a card was discarded into exile: {batch:?}"
    );
    assert_open(seats, &before, &driver, "Fiery Temper");
}
