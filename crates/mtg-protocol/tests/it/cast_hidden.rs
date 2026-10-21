//! Goal: a spell cast from a library during a resolution (the cascade of Bloodbraid Elf) shows every seat the cards exiled, the cast and the spell on the stack, and nothing else of the library: the other cards in it stay unnamed in every seat's events, table and objects, the owner's included, and the order they were bottomed in is a count.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, HiddenEvent, project_events};
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_in};

/// Seat 0 casts Bloodbraid Elf with a library whose second card is the one the cascade finds; every other seat has a library
/// of its own.
fn cascade_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Bloodbraid Elf"])
        .battlefield(0, [mountain(), mountain(), forest(), forest()].map(ready))
        .library_named(
            0,
            [
                "Forest",
                "Lightning Bolt",
                "Giant Growth",
                "Counterspell",
                "Wrath of God",
            ],
        );
    for seat in 1..seats {
        scenario = scenario.library_named(seat, ["Serra Angel", "Doom Blade", "Fireball"]);
    }
    scenario
}

/// A permanent that lets its controller cast the top card of their library.
fn library_key() -> std::sync::Arc<mtg_cards::CardDefinition> {
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Key")
            .with_type(mtg_core::card::CardType::Artifact)
            .with_rules(
                serde_json::from_str(r#"{"Rule":{"rule":{"MayCast":{"from":"Library","what":[]}}}}"#)
                    .unwrap(),
            ),
    )
}

#[rstest]
fn a_cast_from_the_top_of_a_library_shows_every_seat_the_cast_and_the_spell_and_not_the_cards_below(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(mountain()), ready(library_key())])
        .library_named(
            0,
            ["Lightning Bolt", "Giant Growth", "Counterspell", "Wrath of God"],
        );
    for seat in 1..seats {
        scenario = scenario.library_named(seat, ["Serra Angel", "Doom Blade", "Fireball"]);
    }
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Lightning Bolt").unwrap();
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe]).unwrap();
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto)).unwrap();
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let bolt = events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { object, .. } => Some(*object),
            _ => None,
        })
        .expect("the spell was cast");
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    let hidden: BTreeMap<ObjectId, String> = viewers
        .iter()
        .flat_map(|viewer| hidden_in(&after, *viewer))
        .collect();
    assert!(hidden.len() >= 5, "the rest of the libraries is hidden: {hidden:?}");
    assert!(!hidden.contains_key(&bolt));
    for (viewer, batch) in viewers.iter().zip(&batches) {
        assert_clean(batch, &hidden);
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                EventView::Public(Event::SpellCast { object, .. }) if *object == bolt
            )),
            "seat {} sees the cast",
            viewer.index()
        );
        assert!(
            serde_json::to_string(&batch.objects)
                .unwrap()
                .contains("Lightning Bolt"),
            "seat {} sees the spell on the stack",
            viewer.index()
        );
    }
}

#[rstest]
fn every_seat_sees_the_cards_exiled_the_cast_and_the_spell_and_nothing_else_of_the_library(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = cascade_table(seats);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Bloodbraid Elf").unwrap();
    // Every seat passes and the trigger resolves, as far as the question of the cast.
    while driver.decision().unwrap().kind.name() == "priority" {
        driver.pass().unwrap();
    }
    driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe]).unwrap();
    let events = driver.engine().events().to_vec();
    let after = driver.state().clone();
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();

    let exiled: Vec<ObjectId> = events
        .iter()
        .filter_map(|event| match event {
            Event::ZoneChanged {
                object,
                from: Zone::Library(_),
                to: Zone::Exile,
                ..
            } => Some(*object),
            _ => None,
        })
        .collect();
    assert_eq!(exiled.len(), 2, "the land and the card found");
    let cast: Vec<ObjectId> = events
        .iter()
        .filter_map(|event| match event {
            Event::SpellCast { object, .. } => Some(*object),
            _ => None,
        })
        .collect();
    assert_eq!(cast.len(), 2, "the elf and the card cascaded into");
    assert_eq!(cast[1], exiled[1]);

    // What no seat may learn: the cards that are still in a library and were never shown.
    let hidden: BTreeMap<ObjectId, String> = viewers
        .iter()
        .flat_map(|viewer| hidden_in(&after, *viewer))
        .filter(|(id, _)| !exiled.contains(id))
        .collect();
    assert!(hidden.len() >= 6, "the rest of the libraries is hidden: {hidden:?}");

    for (viewer, batch_list) in viewers.iter().zip(&batches) {
        let seat = viewer.index();
        assert_clean(batch_list, &hidden);
        let events_seen = &batch_list.events;
        for card in &exiled {
            assert!(
                events_seen.iter().any(|event| matches!(
                    event,
                    EventView::Public(Event::ZoneChanged { object, to: Zone::Exile, .. }) if object == card
                )),
                "seat {seat} sees {card:?} exiled"
            );
        }
        assert!(
            events_seen.iter().any(|event| matches!(
                event,
                EventView::Public(Event::SpellCast { object, .. }) if *object == cast[1]
            )),
            "seat {seat} sees the cast"
        );
        let everything = serde_json::to_string(&batch_list.objects).unwrap();
        assert!(
            everything.contains("Lightning Bolt"),
            "seat {seat} sees the spell on the stack"
        );
        assert!(
            events_seen
                .iter()
                .any(|event| matches!(event, EventView::Hidden(HiddenEvent::CardsBottomed { count: 1, .. }))),
            "seat {seat} learns that a card went to the bottom, and how many"
        );
        assert!(
            !events_seen
                .iter()
                .any(|event| matches!(event, EventView::Public(Event::LibraryBottomed { .. }))),
            "seat {seat} is not told the order"
        );
    }
}
