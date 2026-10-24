//! Goal: the player a permanent chooses as it enters is public: the question is put to the controller alone, the event that logs the choice reaches every seat whole at two to four seats, and the choice round-trips through its JSON as a `Player` variant of the chosen value.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_protocol::{EventView, project_events};
use mtg_testkit::cards::{island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

#[rstest]
fn the_chosen_player_is_asked_of_the_controller_and_told_to_every_seat_whole(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] pick: usize,
) {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("True-Name Nemesis").unwrap()])
        .battlefield(0, [ready(island()), ready(island()), ready(swamp())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = driver.state().clone();
    let from = driver.engine().events().len();
    driver
        .cast_tapping("True-Name Nemesis", &["Island", "Island", "Swamp"])
        .unwrap();
    for _ in 0..seats {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseOption(_)
        ) {
            break;
        }
        driver.pass().unwrap();
    }
    let decision = driver.decision().unwrap();
    assert!(matches!(decision.kind, DecisionKind::ChooseOption(_)));
    assert_eq!(decision.player, seat(0), "the controller alone is asked");
    driver
        .answer(Answer::ChooseOption(OptionChoice(pick)))
        .unwrap();
    let events = driver.engine().events()[from..].to_vec();
    let entered = events
        .iter()
        .find(|event| matches!(event, Event::Entered { .. }))
        .cloned()
        .expect("the permanent entered with its choice logged");
    let Event::Entered { chosen, .. } = &entered else {
        unreachable!()
    };
    assert_eq!(chosen, &Some(Chosen::Player(seat(pick))));
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in batches.iter().enumerate() {
        assert!(
            batch
                .events
                .iter()
                .any(|told| matches!(told, EventView::Public(seen) if *seen == entered)),
            "seat {viewer} is told who was chosen"
        );
    }
    let json = serde_json::to_value(&entered).unwrap();
    assert_eq!(
        json["Entered"]["chosen"],
        serde_json::json!({ "Player": pick })
    );
    let back: Event = serde_json::from_value(json).unwrap();
    assert_eq!(back, entered);
}
