//! Goal: mana that stays past the step is public like the rest of the pool: every seat sees the events that add it with their lifetime and the pool of the seat that holds it, the lifetime survives the wire form, and nothing of the cards that stay hidden is shown with it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::spend::ManaLasts;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, StateView, project_events};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_in};

/// Seat 0 controls Savage Ventmaw, ready; the other seats hold a Wall and a library.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Savage Ventmaw").unwrap())])
        .library_named(0, ["Lightning Bolt", "Giant Growth", "Counterspell"]);
    for seat in 1..seats {
        scenario = scenario
            .battlefield(seat, [ready(free_creature(&format!("Wall {seat}"), 0, 9))])
            .library_named(seat, ["Serra Angel", "Doom Blade", "Fireball"]);
    }
    scenario
}

#[rstest]
fn kept_mana_is_public_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let scenario = table(seats);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Savage Ventmaw", 1)]).unwrap();
    while !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        || !driver.state().stack().is_empty()
    {
        match driver.decision().unwrap().kind {
            DecisionKind::Priority(_) => driver.pass().unwrap(),
            _ => driver.answer_passively().unwrap(),
        };
    }
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let added: Vec<&Event> = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ManaAdded {
                    lasts: ManaLasts::Turn,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(added.len(), 6, "the trigger adds six mana that stay");
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let hidden: BTreeMap<ObjectId, String> = hidden_in(&after, *viewer);
        assert_clean(batch, &hidden);
        let public: Vec<&Event> = batch
            .events
            .iter()
            .filter_map(|view| match view {
                EventView::Public(event) => Some(event),
                EventView::Hidden(_) => None,
            })
            .collect();
        for event in &added {
            assert!(
                public.contains(event),
                "seat {} sees {event:?}",
                viewer.index()
            );
        }
        let view = StateView::for_player(&after, *viewer);
        let pool = &view.table.players[0].mana;
        assert_eq!(
            pool.total(),
            6,
            "seat {} sees the pool of seat 0",
            viewer.index()
        );
        let kept = pool.kept().expect("the pool keeps mana");
        assert_eq!(kept.turn_end[ManaType::Red.index()], 3);
        assert_eq!(kept.turn_end[ManaType::Green.index()], 3);
    }
}

#[test]
fn the_lifetime_of_mana_is_on_the_wire_and_left_out_for_mana_of_the_step() {
    let kept = Event::ManaAdded {
        player: PlayerId(0),
        mana: ManaType::Red,
        source: None,
        only: Vec::new(),
        lasts: ManaLasts::Turn,
    };
    let plain = Event::ManaAdded {
        player: PlayerId(0),
        mana: ManaType::Red,
        source: None,
        only: Vec::new(),
        lasts: ManaLasts::Step,
    };
    let json = serde_json::to_string(&kept).unwrap();
    assert!(json.contains(r#""lasts":"Turn""#), "{json}");
    assert!(!serde_json::to_string(&plain).unwrap().contains("lasts"));
    assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), kept);
    let emptied = Event::ManaPoolEmptied {
        player: PlayerId(0),
        through: ManaLasts::Combat,
    };
    let json = serde_json::to_string(&emptied).unwrap();
    assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), emptied);
    let old = r#"{"ManaPoolEmptied":{"player":0}}"#;
    assert_eq!(
        serde_json::from_str::<Event>(old).unwrap(),
        Event::ManaPoolEmptied {
            player: PlayerId(0),
            through: ManaLasts::Step
        }
    );
}
