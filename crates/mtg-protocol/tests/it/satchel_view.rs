//! Goal: at two, three and four seats every seat sees each hero's Satchel (its cards and their names) apart from the
//! command zone, the owner's priority offers its cards like cards in hand, a cast card leaves the Satchel's list, and a
//! seat without a Satchel leaves the field out of its JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_protocol::TableView;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Seat 0 has a Grizzly ({1}{G}) in its Satchel and two Forests.
fn driver(seats: usize) -> Driver {
    let scenario = Scenario::new(seats)
        .satchel(0, [creature("Grizzly", 1, &[Color::Green], 2, 2)])
        .battlefield(0, [ready(forest()), ready(forest())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::Main1).unwrap();
    driver
}

#[rstest]
fn every_seat_sees_the_satchel_apart_from_the_command_zone(#[values(2, 3, 4)] seats: usize) {
    let driver = driver(seats);
    let grizzly = driver.state().satchel(seat(0))[0];
    for viewer in 0..seats {
        let view = TableView::of(driver.state(), seat(viewer));
        assert_eq!(view.players[0].satchel, vec![grizzly], "seat {viewer}");
        let command = &view.players[0].command;
        assert_eq!(command.count, 0);
        assert!(!command.cards.clone().unwrap_or_default().contains(&grizzly));
        let objects = mtg_protocol::StateView::for_player(driver.state(), seat(viewer)).objects;
        let object = objects.iter().find(|o| o.id == grizzly).unwrap();
        assert_eq!(object.name, "Grizzly");
        for other in 1..seats {
            let json = serde_json::to_value(&view.players[other]).unwrap();
            assert!(json.get("satchel").is_none(), "left out while empty");
        }
    }
}

#[rstest]
fn the_owner_is_offered_the_satchel_and_a_cast_card_leaves_it(#[values(2, 3, 4)] seats: usize) {
    let mut driver = driver(seats);
    let grizzly = driver.state().satchel(seat(0))[0];
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("seat 0 holds priority");
    };
    assert!(
        options.castable.contains(&grizzly),
        "offered like a card in hand"
    );
    driver.cast("Grizzly").unwrap();
    for viewer in 0..seats {
        let view = TableView::of(driver.state(), seat(viewer));
        assert_eq!(view.players[0].satchel, Vec::new());
        let json = serde_json::to_value(&view.players[0]).unwrap();
        assert!(json.get("satchel").is_none());
    }
}
