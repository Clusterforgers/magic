//! Goal: when a graveyard is shuffled into an empty library under the house rule, no seat is told the new order: the event is shown as a count, the card drawn from it is shown to its owner only, and the cards that went in are gone from every view, while the house rules of the turn and life show to every seat as set.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_core::house::{EmptyLibrary, Handicap, HouseRules};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameState};
use mtg_protocol::{EventView, HiddenEvent, StateView, project_events};
use mtg_testkit::cards::{bear, elf, giant, mountain, swamp};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

fn viewers(players: usize) -> Vec<PlayerId> {
    (0..players).map(PlayerId::from_index).collect()
}

/// A game where seat 0 draws from an empty library with five different cards in the graveyard, played to its first
/// main phase; returns the log, the state before it and the order the graveyard was shuffled into.
fn refilled(players: usize, seed: u64) -> (Vec<Event>, GameState, Vec<ObjectId>) {
    let house = HouseRules {
        empty_library: Some(EmptyLibrary::ReshuffleGraveyard),
        ..HouseRules::default()
    };
    let setup = Scenario::new(players)
        .seed(seed)
        .at(2, 0, Step::Upkeep)
        .library(0, [])
        .graveyard(0, [bear(), elf(), giant(), swamp(), mountain()])
        .house_rules(house)
        .setup()
        .unwrap();
    let before = GameState::from_setup(&setup).unwrap();
    let mut driver = Driver::new(Engine::new(setup).unwrap()).unwrap();
    driver.idle_until(2, Step::Main1).unwrap();
    let events = driver.engine().events().to_vec();
    let order = events
        .iter()
        .find_map(|event| match event {
            Event::LibraryRefilled { order, .. } => Some(order.clone()),
            _ => None,
        })
        .expect("the graveyard was shuffled in");
    (events, before, order)
}

#[rstest]
fn nobody_is_told_the_new_order_of_the_library(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2)] seed: u64,
) {
    let (events, before, order) = refilled(players, seed);
    assert_eq!(order.len(), 5);
    let drawn = *order.last().unwrap();
    let at = events
        .iter()
        .position(|e| matches!(e, Event::LibraryRefilled { .. }))
        .unwrap();
    let batches = project_events(&before, &events, &viewers(players), 0).unwrap();
    for (viewer, batch) in viewers(players).into_iter().zip(&batches) {
        // The shuffle is a count to everybody, the owner included.
        assert_eq!(
            batch.events[at],
            EventView::Hidden(HiddenEvent::LibraryRefilled {
                player: PlayerId(0),
                count: 5
            })
        );
        assert!(
            batch
                .events
                .iter()
                .all(|e| !matches!(e, EventView::Public(Event::LibraryRefilled { .. }))),
            "{viewer:?} was shown the shuffle"
        );
        // The card drawn from the new top is named to its owner and to nobody else.
        let draws: Vec<&EventView> = batch
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    EventView::Public(Event::CardDrawn { .. })
                        | EventView::Hidden(HiddenEvent::CardDrawn { .. })
                )
            })
            .collect();
        let shown = if viewer == PlayerId(0) {
            EventView::Public(Event::CardDrawn {
                player: PlayerId(0),
                object: drawn,
            })
        } else {
            EventView::Hidden(HiddenEvent::CardDrawn {
                player: PlayerId(0),
            })
        };
        assert_eq!(draws, [&shown]);
        // The views after it hold the drawn card for its owner only, and none of the cards under it.
        let seen: BTreeSet<ObjectId> = batch.objects.iter().map(|o| o.id).collect();
        let mine = viewer == PlayerId(0);
        assert_eq!(seen.contains(&drawn), mine, "{viewer:?} and the drawn card");
        for under in order.iter().filter(|card| **card != drawn) {
            assert!(
                !seen.contains(under),
                "{viewer:?} sees a card in the library"
            );
        }
        let json = serde_json::to_string(&batch.events).unwrap();
        assert!(
            !json.contains("\"order\""),
            "the order is on the wire: {json}"
        );
    }
}

#[rstest]
fn the_table_after_the_shuffle_counts_the_library_and_names_none_of_it(
    #[values(2, 3)] players: usize,
) {
    let (events, before, order) = refilled(players, 3);
    let mut state = before;
    for event in &events {
        mtg_engine::apply_event(&mut state, event).unwrap();
    }
    assert_eq!(state.zone(Zone::Library(PlayerId(0))).len(), 4);
    for viewer in viewers(players) {
        let view = StateView::for_player(&state, viewer);
        let zones: Vec<_> = view
            .objects
            .iter()
            .filter(|o| order.contains(&o.id) && matches!(o.zone, Zone::Library(_)))
            .collect();
        assert!(zones.is_empty(), "{viewer:?} sees library cards");
        let seat = &view.table.players[0];
        assert_eq!(
            (seat.library.count, seat.library.cards.is_none()),
            (4, true)
        );
        assert_eq!(seat.graveyard.count, 0);
    }
}

#[test]
fn the_rules_of_the_turn_and_life_show_to_every_seat_as_they_were_set() {
    let house = HouseRules {
        draw_step_cards: Some(2),
        mana_burn: Some(true),
        winning_life: Some(60),
        handicaps: vec![Handicap {
            seat: PlayerId(1),
            starting_life: Some(5),
            opening_hand_size: None,
        }],
        ..HouseRules::default()
    };
    let engine =
        Engine::new(mtg_testkit::fuzz_setup(3, 2).with_house_rules(house.clone())).unwrap();
    for seat in 0..3 {
        let view = StateView::for_player(engine.state(), PlayerId(seat));
        assert_eq!(view.table.house_rules.as_ref(), Some(&house));
        let life: Vec<i32> = view.table.players.iter().map(|p| p.life).collect();
        assert_eq!(life, [20, 5, 20]);
    }
}
