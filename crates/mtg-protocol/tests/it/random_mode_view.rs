//! Goal: the mode an ability picked at random (Umaro, Raging Yeti) is told to every seat at tables of two, three and four seats as part of the public event that put the ability on the stack: each seat sees the same mode, marked as drawn at random, no seat gets a hidden form of the event, and the cards in the hands stay out of it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, project_events};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

#[rstest]
fn the_mode_drawn_at_random_is_public_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .seed(11)
        .at(1, 0, Step::Main1)
        .battlefield_named(0, ["Umaro, Raging Yeti"])
        .hand_named(0, ["Lightning Bolt"])
        .library_named(0, ["Grizzly Bears"; 12]);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.idle_until(1, Step::End).unwrap();
    let events = driver.engine().events().to_vec();
    let picked: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::AbilityPutOnStack(put) if put.choices.random_modes => {
                Some(put.choices.modes.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(picked.len(), 1, "one pick in the log");
    assert_eq!(picked[0].len(), 1, "one mode");
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (seat, batch) in batches.iter().enumerate() {
        let shown: Vec<_> = batch
            .events
            .iter()
            .filter_map(|event| match event {
                EventView::Public(Event::AbilityPutOnStack(put)) if put.choices.random_modes => {
                    Some(put.choices.modes.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(shown, picked, "seat {seat} is told the pick as logged");
        let json = serde_json::to_string(&batch.events).unwrap();
        assert!(
            !json.contains("Hidden\":{\"AbilityPutOnStack"),
            "seat {seat}"
        );
    }
}
