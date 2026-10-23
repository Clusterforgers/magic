//! Goal: an ability whose cost discards a card ("Discard a card: gain life equal to the discarded card's mana value") keeps the rest of every hand hidden: the object the cost took rides on the ability's events as it was and is named to every seat, since the discard names it, and no other card of any hand is named to a seat that may not see it, in the events, the table, the objects or the snapshot.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, StateView, project_events};
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::{assert_clean, hidden_across};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn sorcery(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Seat 0 controls the Altar and holds three cards; every other seat has a hand of its own.
fn table(seats: usize) -> Scenario {
    let ability = r#"{"Activated":{"cost":[{"Discard":{}}],"effects":[{"GainLife":{"amount":{"Stat":{"of":"Paid","stat":"ManaValue"}}}}]}}"#;
    let altar = Arc::new(
        CardDefinition::new("Altar")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(ability).unwrap()),
    );
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 8])
        .battlefield(0, [ready(altar)])
        .hand(
            0,
            [
                sorcery("Counterspell", "{U}{U}"),
                sorcery("Wrath of God", "{2}{W}{W}"),
                sorcery("Doom Blade", "{1}{B}"),
            ],
        );
    let others = [
        ["Fireball", "Opt"],
        ["Brainstorm", "Giant Growth"],
        ["Shock", "Duress"],
    ];
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 8])
            .hand(other, others[other - 1].map(|name| sorcery(name, "{R}")));
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

fn card_in(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

#[rstest]
fn the_card_the_cost_took_is_named_to_every_seat_and_the_rest_of_the_hands_to_none_but_their_owners(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    driver.begin_activation("Altar").unwrap();
    assert_eq!(driver.decision().unwrap().player, seat(0));
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseCards(_)
    ));
    let wrath = card_in(driver.state(), Zone::Hand(seat(0)), "Wrath of God");
    driver
        .answer(Answer::ChooseCards(CardsChoice(vec![wrath])))
        .unwrap();
    let after = driver.state().clone();
    let events = driver.engine().events()[logged..].to_vec();
    let put = events
        .iter()
        .find_map(|event| match event {
            Event::AbilityPutOnStack(put) => Some(put),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        put.choices
            .cost_shots()
            .iter()
            .map(|shot| shot.id)
            .collect::<Vec<_>>(),
        [wrath],
        "the ability remembers the card it took"
    );
    let all: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        assert!(
            batch
                .events
                .iter()
                .any(|view| matches!(view, EventView::Public(Event::AbilityPutOnStack(_)))),
            "{viewer:?} sees the ability on the stack"
        );
        assert!(
            batch_names(batch, "Wrath of God"),
            "{viewer:?} reads the discarded card"
        );
        let mut hidden = hidden_across(&before, &after, *viewer);
        hidden.remove(&wrath);
        assert_clean(batch, &hidden);
        if *viewer != seat(0) {
            for name in ["Counterspell", "Doom Blade"] {
                assert!(
                    !batch_names(batch, name),
                    "{viewer:?} reads {name} of the caster's hand"
                );
            }
        }
        for name in [
            "Fireball",
            "Opt",
            "Brainstorm",
            "Giant Growth",
            "Shock",
            "Duress",
        ] {
            assert!(
                !batch_names(batch, name),
                "{viewer:?} reads {name} of another hand"
            );
        }
    }
    for viewer in &all {
        let value = serde_json::to_value(StateView::for_player(&after, *viewer)).unwrap();
        assert!(
            names(&value, "Wrath of God"),
            "{viewer:?} reads the graveyard"
        );
        assert_eq!(names(&value, "Counterspell"), *viewer == seat(0));
    }
}
