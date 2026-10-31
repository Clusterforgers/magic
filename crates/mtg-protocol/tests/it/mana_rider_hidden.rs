//! Goal: a cast that spends mana carrying a rider (Path of Ancestry) discloses what the table sees and nothing more, at two to four seats and to a spectator: every viewer reads the delayed trigger in the open, and the hands, the libraries and the commander's hidden neighbours stay out of every batch.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::format::Commander;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, project_events};
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
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

/// A creature called `name` of the creature type Elf.
fn elf(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_mana_cost(ManaCost::parse("{G}").unwrap())
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_power_toughness(1, 1),
    )
}

/// Seat 0 holds the Elf Spell and a Secret Plan, has an Elf commander and a Path of Ancestry; the other seats hold cards of
/// their own.
fn table(seats: usize) -> Scenario {
    let commander = Arc::new(
        CardDefinition::new("Elf Chief")
            .with_mana_cost(ManaCost::parse("{G}").unwrap())
            .with_supertype(Supertype::Legendary)
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_power_toughness(2, 2),
    );
    let mut scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [commander])
        .library(0, vec![forest(); 8])
        .hand(0, [elf("Spell"), elf("Secret Plan")])
        .battlefield(0, [ready(card("Path of Ancestry").unwrap())]);
    let others = [["Wrath of God"], ["Fireball"], ["Opt"]];
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 8])
            .hand_named(other, others[other - 1]);
    }
    scenario
}

#[rstest]
fn every_seat_and_a_spectator_reads_the_delayed_trigger_and_no_hidden_card(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let before: GameState = driver.state().clone();
    let logged = driver.engine().events().len();
    driver.cast_tapping("Spell", &["Path of Ancestry"]).unwrap();
    let after = driver.state().clone();
    let events = driver.engine().events()[logged..].to_vec();
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::DelayedTriggerCreated(created)) if created.controller == seat(0)
            )),
            "{viewer:?} reads the trigger the mana set up in the open"
        );
        assert_clean(batch, &hidden_across(&before, &after, *viewer));
        for name in ["Secret Plan", "Wrath of God", "Fireball", "Opt"] {
            assert!(!batch_names(batch, name), "{viewer:?} reads {name}");
        }
    }
}
