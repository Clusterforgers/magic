//! Goal: a Saga that returned transformed is shown to every seat at two, three and four seats as the creature of its back face (its name, types, power, toughness, no Saga subtype, no lore counters, the face number) controlled by the player the Saga was, in a view that round-trips through its JSON, and the events that exile it, return it and turn it over reach every seat whole in order.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, ObjectView, project_events};
use mtg_testkit::saga_cards::transforming_saga;
use mtg_testkit::{Driver, Scenario, ready, with_counters};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// The Saga on seat 0's battlefield with two lore counters, played through its last chapter: the driver before and
/// after, the id of the card and the events of the chapter.
fn returned(seats: usize) -> (GameState, Driver, ObjectId, Vec<Event>) {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [with_counters(
                ready(transforming_saga("Tale")),
                CounterKind::lore(),
                2,
            )],
        )
        .at(1, 0, Step::Upkeep);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let id = driver.id_of("Tale").unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let before = driver.state().clone();
    let mut log = Vec::new();
    for _ in 0..100 {
        let ordering = matches!(
            driver.decision().unwrap().kind,
            DecisionKind::OrderTriggers(_)
        );
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() && !ordering {
            break;
        }
        log.extend(driver.answer_passively().unwrap());
    }
    (before, driver, id, log)
}

#[rstest]
fn the_returned_saga_is_its_back_face_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let (_, driver, id, _) = returned(seats);
    for viewer in 0..seats {
        let view = ObjectView::of(driver.state(), seat(viewer), id).unwrap();
        assert_eq!(
            (view.name.as_str(), view.face, view.power, view.toughness),
            ("Hero of Tale", Some(1), Some(3), Some(3))
        );
        assert_eq!(view.controller, seat(0));
        assert!(view.types.contains(&CardType::Creature));
        assert!(view.subtypes.iter().all(|subtype| subtype.0 != "Saga"));
        assert!(view.counters.is_empty(), "{:?}", view.counters);
        assert_eq!(view.mana_cost, None, "the back face prints no cost");
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["face"], 1);
        let back: ObjectView = serde_json::from_value(json).unwrap();
        assert_eq!(back, view);
    }
}

#[rstest]
fn the_saga_before_the_last_chapter_is_the_front_face_to_every_seat(
    #[values(2, 3, 4)] seats: usize,
) {
    let (before, _, id, _) = returned(seats);
    for viewer in 0..seats {
        let view = ObjectView::of(&before, seat(viewer), id).unwrap();
        assert_eq!((view.name.as_str(), view.face), ("Tale", None));
        assert!(view.subtypes.iter().any(|subtype| subtype.0 == "Saga"));
        assert_eq!(view.mana_cost.unwrap().to_string(), "{1}{W}");
    }
}

#[rstest]
fn the_events_of_the_return_reach_every_seat_whole_and_in_order(#[values(2, 3, 4)] seats: usize) {
    let (before, driver, id, log) = returned(seats);
    let ours: Vec<Event> = log
        .into_iter()
        .filter(|event| event.touched_objects().contains(&id))
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged { .. } | Event::Entered { .. } | Event::Transformed { .. }
            )
        })
        .collect();
    assert_eq!(ours.len(), 4, "{ours:?}");
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &ours, &viewers, 1).unwrap();
    assert_eq!(batches.len(), seats);
    let _ = driver;
    for batch in batches {
        let shown: Vec<EventView> = batch.events;
        assert_eq!(shown.len(), ours.len());
        assert!(matches!(
            shown.last(),
            Some(EventView::Public(Event::Transformed { face: Some(1), .. }))
        ));
    }
}
