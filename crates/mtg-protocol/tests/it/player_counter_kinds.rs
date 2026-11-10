//! Goal: every kind of counter a player holds (rad, poison, energy, experience, ticket and any other) is in the view of every seat and of a spectator with its count, whichever seat holds it, both on a table set up with it and in the table of the batch that puts it there.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::counters::CountersChanged;
use mtg_protocol::{StateView, project_events};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn kind(name: &str) -> CounterKind {
    match name {
        "poison" => CounterKind::Poison,
        other => CounterKind::Other(other.to_owned()),
    }
}

/// The views of every seat and of a spectator.
fn every_view(driver: &Driver, seats: usize) -> Vec<StateView> {
    (0..seats)
        .map(|seat| StateView::for_player(driver.state(), PlayerId::from_index(seat)))
        .chain([StateView::for_spectator(driver.state())])
        .collect()
}

#[rstest]
fn a_players_counters_of_every_kind_are_in_every_view(
    #[values("rad", "poison", "energy", "experience", "ticket", "acorn")] name: &str,
    #[values(2, 4)] seats: usize,
    #[values(0, 1)] holder: usize,
) {
    let scenario = Scenario::new(seats).counters(holder, kind(name), 7);
    let driver = Driver::from_scenario(scenario).unwrap();
    for view in every_view(&driver, seats) {
        for player in &view.table.players {
            let expected = if player.seat == PlayerId::from_index(holder) {
                vec![(kind(name), 7)]
            } else {
                Vec::new()
            };
            assert_eq!(player.counters, expected, "{name} on seat {holder}");
        }
    }
}

#[rstest]
fn the_batch_that_puts_counters_on_a_player_carries_them_to_every_seat(
    #[values("rad", "poison", "energy", "experience")] name: &str,
    #[values(2, 3, 4)] seats: usize,
) {
    let driver = Driver::from_scenario(Scenario::new(seats).counters(1, kind(name), 2)).unwrap();
    let events = [Event::CountersChanged(CountersChanged {
        on: Target::Player(PlayerId(1)),
        counter: kind(name),
        delta: 3,
    })];
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(driver.state(), &events, &viewers, 0).unwrap();
    assert_eq!(batches.len(), seats);
    for batch in &batches {
        assert_eq!(batch.table.players[1].counters, [(kind(name), 5)]);
    }
}
