//! Goal: a card exiled face down that no player may look at (CR 406.3) lies in exile for every seat to see that it is there and no seat learns which card it is, its owner and the player whose effect exiled it included: the move is told hidden, the batches and the resumed views never name it, the replay agrees; a rule that grants a look ("you may look at the cards exiled with ~") shows it to the seats it names alone, and only while it is in force; at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, replay};
use mtg_protocol::{EventBatch, EventView, HiddenEvent, StateView, project_events};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::{assert_clean, hidden_in};

const SECRETS: [&str; 2] = ["Secret One", "Secret Two"];

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(PlayerId::from_index).collect()
}

/// A Courier with "{0}: exile the top two cards of your library face down", and, with `grant`, "you may look at the cards
/// exiled with this" for the players the grant names.
fn courier(grant: Option<&str>) -> Arc<CardDefinition> {
    let exile: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":[],"effects":[{"ExileTop":{"count":2,"as":"sealed","sealed":true}}]}}"#,
    )
    .unwrap();
    let mut card = CardDefinition::new("Courier")
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1)
        .with_rules(exile);
    if let Some(who) = grant {
        let look: Ability = serde_json::from_str(&format!(
            r#"{{"Rule":{{"rule":{{"LookAtExiled":{{"who":"{who}","what":[{{"ExiledWith":"This"}}]}}}}}}}}"#
        ))
        .unwrap();
        card = card.with_rules(look);
    }
    Arc::new(card)
}

/// Seat 0 controls the Courier over a library whose top two cards are the secrets.
fn table(seats: usize, grant: Option<&str>) -> Scenario {
    // The first cards of the list are the top of the library.
    let mut library: Vec<_> = SECRETS
        .iter()
        .map(|name| free_creature(name, 2, 2))
        .collect();
    library.extend((0..4).map(|i| free_creature(&format!("Pad {i}"), 1, 1)));
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(courier(grant))])
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

/// Whether any part of the batch holds a secret's name.
fn batch_names(batch: &EventBatch) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| SECRETS.iter().any(|name| names(part, name)))
}

/// Seat 0 activates the Courier and lets it resolve: the state before, the events, the state after and the sealed cards.
fn seal(scenario: Scenario) -> (GameState, Vec<Event>, GameState, Vec<ObjectId>) {
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Courier").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let sealed: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .collect();
    assert_eq!(sealed.len(), 2, "both cards are exiled face down");
    for id in &sealed {
        let name = &driver.state().object(*id).unwrap().card().name;
        assert!(SECRETS.contains(&name.as_str()), "the top cards are sealed");
    }
    (
        before,
        driver.engine().events().to_vec(),
        driver.state().clone(),
        sealed,
    )
}

#[rstest]
fn no_seat_learns_which_cards_were_sealed(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(seats, None);
    let (before, events, after, sealed) = seal(scenario.clone());
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        for id in &sealed {
            let object = batch.objects.iter().find(|object| object.id == *id);
            let object = object.expect("every viewer sees that the card is in exile");
            assert!(object.face_down && object.name.is_empty(), "{viewer:?}");
            assert_eq!(object.owner, seat(0));
        }
        assert!(!batch_names(batch), "no secret is named to {viewer:?}");
        let hidden_moves = batch
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    EventView::Hidden(HiddenEvent::ZoneChanged {
                        to: Zone::Exile,
                        cause: ZoneCause::Sealed { .. },
                        ..
                    })
                )
            })
            .count();
        assert_eq!(hidden_moves, 2, "both moves are hidden from {viewer:?}");
        assert_clean(batch, &hidden_in(&after, *viewer));
    }
    let again = replay(&scenario.setup().unwrap(), &events).unwrap();
    for viewer in &all {
        for state in [&after, &again] {
            let view = serde_json::to_value(StateView::for_player(state, *viewer)).unwrap();
            assert!(!SECRETS.iter().any(|name| names(&view, name)), "{viewer:?}");
        }
    }
}

#[rstest]
#[case::you("You", &[0])]
#[case::opponents("Opponents", &[1, 2, 3])]
fn a_grant_shows_the_sealed_cards_to_the_seats_it_names_while_it_is_in_force(
    #[values(2, 3, 4)] seats: usize,
    #[case] who: &str,
    #[case] lookers: &[usize],
) {
    let (_, _, mut after, sealed) = seal(table(seats, Some(who)));
    let looks = |state: &GameState, viewer: PlayerId| {
        let view = serde_json::to_value(StateView::for_player(state, viewer)).unwrap();
        SECRETS.iter().all(|name| names(&view, name))
    };
    for viewer in viewers(seats) {
        let named = lookers.iter().any(|n| *n < seats && seat(*n) == viewer);
        assert_eq!(looks(&after, viewer), named, "{viewer:?}");
        assert_eq!(after.may_look_at(viewer, sealed[0]), named);
    }
    // The Courier leaving ends the grant: the cards are sealed for every seat again.
    let courier = after.zone(Zone::Battlefield)[0];
    let leave = Event::ZoneChanged {
        object: courier,
        from: Zone::Battlefield,
        to: Zone::Graveyard(seat(0)),
        cause: ZoneCause::Destroyed,
    };
    mtg_engine::fold::apply_event(&mut after, &leave).unwrap();
    for viewer in viewers(seats) {
        assert!(!looks(&after, viewer), "{viewer:?}");
    }
}
