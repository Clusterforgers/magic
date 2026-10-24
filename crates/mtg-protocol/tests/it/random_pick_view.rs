//! Goal: a card taken at random from a graveyard is told to every seat as it was logged, at tables of two, three and four seats: the pick is public (the cards of a graveyard are seen by all), no seat gets a hidden form of it, the card it names is the one that moved to the hand, and a pick among cards of a hand or a library is refused by the engine, so no hidden form is needed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, project_events};
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

#[rstest]
fn a_pick_at_random_is_public_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .seed(4)
        .hand_named(0, ["Surreal Memoir"])
        .battlefield(0, (0..4).map(|_| ready(mountain())))
        .graveyard_named(0, ["Lightning Bolt", "Giant Growth", "Counterspell"]);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Surreal Memoir").unwrap();
    for _ in 0..100 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let events = driver.engine().events().to_vec();
    let picked: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::ChosenAtRandom { cards } => Some(cards.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(picked.len(), 1, "one pick in the log");
    let name = driver.state().object(picked[0][0]).unwrap().card().name.clone();
    assert!(["Lightning Bolt", "Giant Growth", "Counterspell"].contains(&name.as_str()));
    let hand = driver.state().zone(mtg_engine::zones::Zone::Hand(PlayerId(0)));
    assert!(hand.contains(&picked[0][0]), "the card picked is the one in the hand");
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (seat, batch) in batches.iter().enumerate() {
        let shown = batch
            .events
            .iter()
            .filter(|event| matches!(event, EventView::Public(Event::ChosenAtRandom { .. })))
            .count();
        assert_eq!(shown, 1, "seat {seat} is told the pick as logged");
        let json = serde_json::to_string(&batch.events).unwrap();
        assert!(!json.contains("Hidden\":{\"ChosenAtRandom"), "seat {seat}");
    }
}
