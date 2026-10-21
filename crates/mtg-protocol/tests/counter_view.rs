//! Goal: the counters on a player are in every seat's view of the table alike, in a fixed order of kinds and left out of the JSON while there are none; the events that put counters on a permanent or a player and take them off reach every seat as logged, and the table after them carries the new totals of the player and the new counters of the permanent; a lost player's counters stay in the view, and the loss for poison is named.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::counters::CountersChanged;
use mtg_engine::player::LossReason;
use mtg_engine::{Event, apply_event};
use mtg_protocol::{EventView, StateView, project_events};
use mtg_testkit::cards::{bear, free_creature};
use mtg_testkit::{Driver, Scenario, ready, with_counters};
use rstest::rstest;
use serde_json::json;

const ENERGY: fn() -> CounterKind = || CounterKind::Other("energy".into());

/// Seat 0 has three poison counters and two energy counters and controls a Bear with a +1/+1 counter; the other seats
/// have none.
fn table(seats: usize) -> Driver {
    let scenario = Scenario::new(seats)
        .counters(0, ENERGY(), 2)
        .counters(0, CounterKind::Poison, 3)
        .battlefield(
            0,
            [with_counters(ready(bear()), CounterKind::PlusOnePlusOne, 1)],
        )
        .battlefield(1, [ready(free_creature("Ogre", 4, 4))]);
    Driver::from_scenario(scenario).unwrap()
}

fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..seats).map(PlayerId::from_index).collect()
}

#[rstest]
fn every_seat_sees_the_counters_of_every_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] viewer: usize,
) {
    let driver = table(seats);
    let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
    assert_eq!(
        view.table.players[0].counters,
        [(CounterKind::Poison, 3), (ENERGY(), 2)],
        "in the order of the kinds"
    );
    for player in &view.table.players[1..] {
        assert_eq!(player.counters, []);
    }
}

#[rstest]
fn the_json_leaves_counters_out_while_there_are_none(#[values(2, 3)] seats: usize) {
    let driver = table(seats);
    let view = serde_json::to_value(StateView::for_player(driver.state(), PlayerId(1))).unwrap();
    let players = view["players"].as_array().unwrap();
    assert_eq!(
        players[0]["counters"],
        json!([["poison", 3], [{"Other": "energy"}, 2]])
    );
    assert!(players[1].get("counters").is_none());
}

#[rstest]
fn counter_events_reach_every_seat_as_logged_and_the_table_after_them_shows_the_totals(
    #[values(2, 3, 4)] seats: usize,
) {
    let driver = table(seats);
    let bear = driver.target_named("Grizzly Bears").unwrap();
    let events = [
        Event::CountersChanged(CountersChanged {
            on: Target::Player(PlayerId(1)),
            counter: CounterKind::Poison,
            delta: 2,
        }),
        Event::CountersChanged(CountersChanged {
            on: Target::Player(PlayerId(0)),
            counter: CounterKind::Poison,
            delta: -1,
        }),
        Event::CountersChanged(CountersChanged {
            on: bear,
            counter: CounterKind::MinusOneMinusOne,
            delta: 1,
        }),
    ];
    let batches = project_events(driver.state(), &events, &viewers(seats), 0).unwrap();
    assert_eq!(batches.len(), seats);
    for batch in &batches {
        let seen: Vec<&Event> = batch
            .events
            .iter()
            .map(|view| match view {
                EventView::Public(event) => event,
                EventView::Hidden(hidden) => panic!("{hidden:?}"),
            })
            .collect();
        assert_eq!(seen, events.iter().collect::<Vec<_>>());
        assert_eq!(
            batch.table.players[0].counters,
            [(CounterKind::Poison, 2), (ENERGY(), 2)]
        );
        assert_eq!(batch.table.players[1].counters, [(CounterKind::Poison, 2)]);
        let bear = batch
            .objects
            .iter()
            .find(|o| o.name == "Grizzly Bears")
            .unwrap();
        assert_eq!(
            bear.counters,
            [
                (CounterKind::PlusOnePlusOne, 1),
                (CounterKind::MinusOneMinusOne, 1)
            ]
        );
    }
}

#[test]
fn a_player_lost_to_poison_keeps_the_counters_in_the_view_and_the_reason_is_named() {
    let driver = table(2);
    let mut state = driver.state().clone();
    for event in [
        Event::CountersChanged(CountersChanged {
            on: Target::Player(PlayerId(0)),
            counter: CounterKind::Poison,
            delta: 7,
        }),
        Event::PlayerLost {
            player: PlayerId(0),
            reason: LossReason::Poison,
        },
    ] {
        apply_event(&mut state, &event).unwrap();
    }
    let view = StateView::for_player(&state, PlayerId(1));
    let seat = &view.table.players[0];
    assert_eq!(seat.lost, Some(LossReason::Poison));
    assert_eq!(seat.counters, [(CounterKind::Poison, 10), (ENERGY(), 2)]);
    assert_eq!(serde_json::to_value(&seat.lost).unwrap(), json!("Poison"));
}
