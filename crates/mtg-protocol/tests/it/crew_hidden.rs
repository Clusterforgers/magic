//! Goal: crewing a Vehicle and saddling a Mount disclose what the table sees and nothing more, at two to four seats and to a spectator. Every seat reads the same `Crewed` event naming the permanent, its controller and the creatures tapped to pay in the open, and no card of any hand or library is named.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, project_events};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::{assert_clean, hidden_across};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
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

/// A 4/4 Vehicle with Crew 1, or a Mount with Saddle 1, named `name`.
fn rig(name: &str, vehicle: bool) -> Arc<CardDefinition> {
    let keyword = |json: &str| serde_json::from_str::<Ability>(json).unwrap();
    let card = CardDefinition::new(name).with_power_toughness(4, 4);
    Arc::new(if vehicle {
        card.with_subtype("Vehicle")
            .with_type(CardType::Artifact)
            .with_rules(keyword(r#"{"Keyword":{"Crew":1}}"#))
    } else {
        card.with_subtype("Mount")
            .with_type(CardType::Creature)
            .with_rules(keyword(r#"{"Keyword":{"Saddle":1}}"#))
    })
}

/// Seat 0 controls the rig and a Rider and holds a Counterspell; the other seats hold cards of their own.
fn table(seats: usize, vehicle: bool) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 8])
        .hand_named(0, ["Counterspell"])
        .battlefield(
            0,
            [
                ready(rig("Rig", vehicle)),
                ready(free_creature("Rider", 1, 1)),
            ],
        );
    let others = [["Wrath of God"], ["Fireball"], ["Opt"]];
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 8])
            .hand_named(other, others[other - 1]);
    }
    scenario
}

/// Activates the crew or saddle ability of the Rig and lets it resolve.
fn activate(driver: &mut Driver) {
    let offered = driver.offered_abilities("Rig").unwrap();
    driver.begin_ability("Rig", offered[0]).unwrap();
    for _ in 0..40 {
        match driver.decision().unwrap().kind.clone() {
            DecisionKind::ChooseCards(options) => {
                let ids = options.from.iter().take(options.min as usize).copied();
                driver
                    .answer(Answer::ChooseCards(CardsChoice(ids.collect())))
                    .unwrap();
            }
            _ if driver.state().stack().is_empty() => return,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the activation did not resolve");
}

#[rstest]
fn every_seat_and_a_spectator_reads_who_crewed_or_saddled_and_no_hidden_card(
    #[values(true, false)] vehicle: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats, vehicle)).unwrap();
    if !vehicle {
        driver.pass_until(Step::Main1).unwrap();
    }
    let before: GameState = driver.state().clone();
    let logged = driver.engine().events().len();
    activate(&mut driver);
    let after = driver.state().clone();
    let events = driver.engine().events()[logged..].to_vec();
    let rig = driver.id_of("Rig").unwrap();
    let rider = driver.id_of("Rider").unwrap();
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::Crewed { object, player, creatures, saddle })
                    if *object == rig
                        && *player == seat(0)
                        && creatures == &[rider]
                        && *saddle != vehicle
            )),
            "{viewer:?} reads the crew in the open"
        );
        assert_clean(batch, &hidden_across(&before, &after, *viewer));
        for name in ["Counterspell", "Wrath of God", "Fireball", "Opt"] {
            assert!(!batch_names(batch, name), "{viewer:?} reads {name}");
        }
    }
}
