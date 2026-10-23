//! Goal: the turn view names the monarch to every seat at two to four seats and leaves the field out while nobody is, the event that makes a player the monarch and the exile of a card until an opponent becomes it reach every seat whole, and a table view with a monarch round-trips through its JSON.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::turn::Step;
use mtg_engine::{Event, apply_event};
use mtg_protocol::{EventView, HiddenEvent, TableView, project_events};
use mtg_testkit::cards::{island, monarcher};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

#[rstest]
fn the_turn_view_names_the_monarch_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(Scenario::new(seats)).unwrap();
    let mut state = driver.state().clone();
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        assert_eq!(view.turn.monarch, None);
        let json = serde_json::to_value(&view).unwrap();
        assert!(
            json["turn"].get("monarch").is_none(),
            "left out while nobody is"
        );
    }
    let before = state.clone();
    let event = Event::MonarchChanged { player: seat(1) };
    apply_event(&mut state, &event).unwrap();
    for viewer in 0..seats {
        let view = TableView::of(&state, seat(viewer));
        assert_eq!(view.turn.monarch, Some(seat(1)));
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["turn"]["monarch"], 1);
        let back: TableView = serde_json::from_value(json).unwrap();
        assert_eq!(back, view);
    }
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, std::slice::from_ref(&event), &viewers, 0).unwrap();
    for batch in &batches {
        assert!(
            batch
                .events
                .iter()
                .any(|told| matches!(told, EventView::Public(seen) if *seen == event)),
            "every seat is told who became the monarch"
        );
    }
}

/// An enchantment that gains its controller 1 life whenever any player becomes the monarch.
fn watcher() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"BecomesMonarch":{}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Enchantment)
            .with_rules(ability),
    )
}

#[rstest]
fn a_trigger_on_becoming_the_monarch_reaches_every_seat_whole_and_the_draw_that_follows_stays_hidden(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .hand(0, [monarcher("Crown")])
        .battlefield(0, [ready(island()), ready(watcher())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = driver.state().clone();
    let from = driver.engine().events().len();
    driver.cast("Crown").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    driver.idle_until(1, Step::End).unwrap();
    let events = driver.engine().events()[from..].to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in batches.iter().enumerate() {
        let told = &batch.events;
        assert!(
            told.iter().any(|view| matches!(
                view,
                EventView::Public(Event::MonarchChanged { player }) if *player == seat(0)
            )),
            "seat {viewer} is told who became the monarch"
        );
        assert!(
            told.iter().any(|view| matches!(
                view,
                EventView::Public(Event::AbilityTriggered(t))
                    if t.cause.subjects == [Target::Player(seat(0))]
            )),
            "seat {viewer} sees the trigger whole, naming the new monarch"
        );
        let drew = told.iter().find_map(|view| match view {
            EventView::Public(Event::CardDrawn { player, .. })
            | EventView::Hidden(HiddenEvent::CardDrawn { player }) => {
                Some((*player, matches!(view, EventView::Public(_))))
            }
            _ => None,
        });
        assert_eq!(
            drew,
            Some((seat(0), viewer == 0)),
            "the monarch drew at the end step and only they see the card"
        );
    }
}

#[rstest]
fn a_card_exiled_until_an_opponent_becomes_the_monarch_is_told_to_every_seat_whole(
    #[values(2, 3, 4)] seats: usize,
) {
    let plains = || ready(mtg_testkit::cards::plains());
    let scenario = Scenario::new(seats)
        .hand(0, [mtg_testkit::corpus::card("Palace Jailer").unwrap()])
        .battlefield(0, [plains(), plains(), plains(), plains()])
        .battlefield(1, [ready(mtg_testkit::cards::free_creature("Wall", 0, 5))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = driver.state().clone();
    let from = driver.engine().events().len();
    driver.cast("Palace Jailer").unwrap();
    for _ in 0..30 {
        match &driver.decision().unwrap().kind {
            mtg_engine::decision::DecisionKind::ChooseTargets(_) => {
                let wall = driver.target_named("Wall").unwrap();
                driver.choose_targets(&[wall]).unwrap();
            }
            mtg_engine::decision::DecisionKind::OrderTriggers(options) => {
                let order: Vec<usize> = (0..options.triggers.len()).collect();
                driver.order_triggers(&order).unwrap();
            }
            _ if driver.state().stack().is_empty() => break,
            _ => {
                driver.pass().unwrap();
            }
        }
    }
    let events = driver.engine().events()[from..].to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in batches.iter().enumerate() {
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::ZoneChanged {
                    cause: mtg_engine::events::ZoneCause::ExiledUntilMonarch { by },
                    ..
                }) if *by == seat(0)
            )),
            "seat {viewer} is told the Wall was exiled until an opponent becomes the monarch"
        );
        assert!(
            batch.events.iter().any(|view| matches!(
                view,
                EventView::Public(Event::MonarchChanged { player }) if *player == seat(0)
            )),
            "seat {viewer} is told who became the monarch"
        );
    }
}
