//! Goal: a card a permanent hides with hideaway lies in exile for every seat to see that it is there, and only its owner sees which card it is: the other seats and a spectator get an object with no characteristics, the move is told to them without the card, nothing in their batches names the card or the cards put on the bottom, and the log never shows the choice.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, HiddenEvent, ObjectView, StateView, project_events};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::{assert_clean, hidden_in};

const SECRET: &str = "Secret Wisp";
const LOOKED: [&str; 3] = ["Looked B", "Looked C", "Looked D"];

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(PlayerId::from_index).collect()
}

/// Seat 0 holds Mosswort Bridge over a library whose top four cards (from either end) are the secret and three more.
fn table(seats: usize) -> Scenario {
    let mut library = vec![free_creature("Pad 0", 1, 1), free_creature("Pad 1", 1, 1)];
    library.push(free_creature(SECRET, 2, 2));
    library.extend(LOOKED.iter().map(|name| free_creature(name, 1, 1)));
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Mosswort Bridge"])
        .library(0, library);
    for other in 1..seats {
        scenario = scenario.library(
            other,
            (0..6).map(|i| free_creature(&format!("Foe {other} {i}"), 1, 1)),
        );
    }
    scenario
}

/// Whether the text of `value` holds `name` anywhere.
fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

/// Whether any part of the batch holds the card's name.
fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

fn object_of(batch: &EventBatch, id: ObjectId) -> Option<&ObjectView> {
    batch.objects.iter().find(|object| object.id == id)
}

/// Plays the Bridge and hides the secret card.
fn hide(driver: &mut Driver) -> ObjectId {
    driver.play_land("Mosswort Bridge").unwrap();
    for _ in 0..20 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass().unwrap();
            }
            DecisionKind::ChooseCards(options) => {
                let secret = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| driver.state().object(*id).unwrap().card().name == SECRET)
                    .expect("the secret card is among the top four");
                driver
                    .answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
                        secret,
                    ])))
                    .unwrap();
                return secret;
            }
            other => panic!("unexpected question: {}", other.name()),
        }
    }
    panic!("the choice was never asked");
}

#[rstest]
fn only_the_owner_sees_which_card_was_hidden(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(seats);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let id = hide(&mut driver);
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let owner = *viewer == seat(0);
        let object = object_of(batch, id).expect("every viewer sees that the card is in exile");
        assert_eq!(object.zone, Zone::Exile);
        assert_eq!(object.owner, seat(0));
        assert!(object.face_down, "the card lies face down for {viewer:?}");
        assert!(
            batch.table.exile.cards.as_ref().unwrap().contains(&id),
            "it is listed in exile for {viewer:?}"
        );
        assert_eq!(
            batch_names(batch, SECRET),
            owner,
            "who is told the name: {viewer:?}"
        );
        if owner {
            assert_eq!(object.name, SECRET);
            assert!(batch.events.iter().any(|event| matches!(
                event,
                EventView::Public(Event::ZoneChanged { object, cause: ZoneCause::Hideaway { .. }, .. })
                    if *object == id
            )));
        } else {
            assert_eq!(object.name, "");
            assert!(object.types.is_empty() && object.abilities.is_empty());
            assert_eq!(object.power, None);
            assert!(batch.events.iter().any(|event| matches!(
                event,
                EventView::Hidden(HiddenEvent::ZoneChanged {
                    from: Zone::Library(from),
                    to: Zone::Exile,
                    cause: ZoneCause::Hideaway { .. },
                }) if *from == seat(0)
            )));
            for looked in LOOKED {
                assert!(
                    !batch_names(batch, looked),
                    "{looked} is not named to {viewer:?}"
                );
            }
            assert_clean(batch, &hidden_in(&after, *viewer));
        }
    }
    // The state a client resumes from is told the same way.
    for viewer in &all {
        let view = StateView::for_player(&after, *viewer);
        let object = view.objects.iter().find(|object| object.id == id).unwrap();
        assert_eq!(object.name == SECRET, *viewer == seat(0));
        assert_eq!(
            names(&serde_json::to_value(&view).unwrap(), SECRET),
            *viewer == seat(0)
        );
    }
}
