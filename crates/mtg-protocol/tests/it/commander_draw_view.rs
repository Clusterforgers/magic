//! Goal: the seats that did not draw learn no more from a commander that would be drawn than from any commander moved out of a library, at 2 to 6 seats: a yes shows every seat the one move from the library to the command zone and no draw (hidden or whole), a no shows the other seats only that a card was drawn while the owner sees which, and the question reaches the owner alone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::ZoneCause;
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, HiddenEvent, project_events};
use mtg_testkit::cards::free_creature;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

const OWNER: PlayerId = PlayerId(0);

/// A Commander table whose seat 0 has the commander on top of a library of creatures, and the state before it draws.
fn table(seats: usize) -> (Driver, ObjectId, GameState) {
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .library(0, (0..6).map(|n| free_creature(&format!("Base {n}"), 1, 1)));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let object = driver.state().commanders().next().unwrap();
    driver
        .manual(ManualAction::Move {
            object,
            to: ManualZone::LibraryTop,
        })
        .unwrap();
    driver.answer(Answer::YesNo(YesNoChoice(false))).unwrap();
    let before = driver.state().clone();
    (driver, object, before)
}

fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..seats).map(PlayerId::from_index).collect()
}

#[rstest]
fn a_commander_sent_to_the_command_zone_in_place_of_a_draw_shows_every_seat_one_move_and_no_draw(
    #[values(2, 3, 4, 5, 6)] seats: usize,
) {
    let (mut driver, object, before) = table(seats);
    let start = driver.engine().events().len();
    driver.manual(ManualAction::Draw { count: 1 }).unwrap();
    // Only the owner is asked, and the question names the commander and nothing about the rest of the library.
    let question = driver.decision().unwrap();
    assert_eq!(question.player, OWNER);
    assert!(matches!(
        &question.kind,
        DecisionKind::YesNo(options)
            if options.source == object && options.text == "Put Cmd into the command zone instead of drawing it?"
    ));
    driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
    let events = driver.engine().events()[start..].to_vec();
    let moved = Event::ZoneChanged {
        object,
        from: Zone::Library(OWNER),
        to: Zone::Command(OWNER),
        cause: ZoneCause::Moved,
    };
    for batch in project_events(&before, &events, &viewers(seats), 0).unwrap() {
        let drawn = batch.events.iter().any(|view| {
            matches!(
                view,
                EventView::Hidden(HiddenEvent::CardDrawn { .. })
                    | EventView::Public(Event::CardDrawn { .. })
            )
        });
        assert!(!drawn, "{:?}", batch.events);
        let moves: Vec<&EventView> = batch
            .events
            .iter()
            .filter(|view| {
                matches!(
                    view,
                    EventView::Public(Event::ZoneChanged { .. })
                        | EventView::Hidden(HiddenEvent::ZoneChanged { .. })
                )
            })
            .collect();
        assert_eq!(
            moves,
            [&EventView::Public(moved.clone())],
            "one public move"
        );
    }
}

#[rstest]
fn a_refused_commander_is_a_hidden_draw_to_the_other_seats_and_a_whole_one_to_its_owner(
    #[values(2, 3, 4, 5, 6)] seats: usize,
) {
    let (mut driver, object, before) = table(seats);
    let start = driver.engine().events().len();
    driver.manual(ManualAction::Draw { count: 1 }).unwrap();
    driver.answer(Answer::YesNo(YesNoChoice(false))).unwrap();
    let events = driver.engine().events()[start..].to_vec();
    let batches = project_events(&before, &events, &viewers(seats), 0).unwrap();
    for (seat, batch) in batches.iter().enumerate() {
        let draws: Vec<&EventView> = batch
            .events
            .iter()
            .filter(|view| {
                matches!(
                    view,
                    EventView::Hidden(HiddenEvent::CardDrawn { .. })
                        | EventView::Public(Event::CardDrawn { .. })
                )
            })
            .collect();
        let expected = if seat == 0 {
            EventView::Public(Event::CardDrawn {
                player: OWNER,
                object,
            })
        } else {
            EventView::Hidden(HiddenEvent::CardDrawn { player: OWNER })
        };
        assert_eq!(draws, [&expected], "seat {seat}");
        assert!(
            !batch
                .events
                .iter()
                .any(|view| matches!(view, EventView::Public(Event::ZoneChanged { .. }))),
            "seat {seat}: {:?}",
            batch.events
        );
    }
}
