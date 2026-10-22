//! Goal: an impulse draw (Reckless Impulse) shows every seat the cards exiled from the top of the library, the land played from exile and the spell cast from exile, and nothing else of the library: the cards below stay unnamed in every seat's events, table and objects, the owner's included.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, project_events};
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_in};

/// Seat 0 holds Reckless Impulse over four Mountains with a library whose first two cards are the ones exiled; every other
/// seat has a library of its own.
fn impulse_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Reckless Impulse"])
        .battlefield(0, (0..4).map(|_| ready(mountain())))
        .library_named(
            0,
            [
                "Lightning Bolt",
                "Forest",
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

#[rstest]
fn every_seat_sees_the_cards_exiled_the_land_played_and_the_spell_cast_and_nothing_else_of_the_library(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = impulse_table(seats);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Reckless Impulse").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    driver.play_land("Forest").unwrap();
    driver.begin_cast("Lightning Bolt").unwrap();
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe]).unwrap();
    driver
        .answer(mtg_engine::decision::Answer::PayMana(
            mtg_engine::decision::PayChoice::Auto,
        ))
        .unwrap();
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
    assert_eq!(exiled.len(), 2, "the two cards of the top");
    let hidden: BTreeMap<ObjectId, String> = viewers
        .iter()
        .flat_map(|viewer| hidden_in(&after, *viewer))
        .filter(|(id, _)| !exiled.contains(id))
        .collect();
    assert!(
        hidden.len() >= 6,
        "the rest of the libraries is hidden: {hidden:?}"
    );

    for (viewer, batch) in viewers.iter().zip(&batches) {
        let seat = viewer.index();
        assert_clean(batch, &hidden);
        for card in &exiled {
            assert!(
                batch.events.iter().any(|event| matches!(
                    event,
                    EventView::Public(Event::ZoneChanged { object, to: Zone::Exile, .. }) if object == card
                )),
                "seat {seat} sees {card:?} exiled"
            );
        }
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                EventView::Public(Event::LandPlayed { object, .. }) if *object == exiled[1]
            )),
            "seat {seat} sees the land played from exile"
        );
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                EventView::Public(Event::SpellCast { object, .. }) if *object == exiled[0]
            )),
            "seat {seat} sees the spell cast from exile"
        );
        let everything = serde_json::to_string(&batch.objects).unwrap();
        for name in ["Lightning Bolt", "Forest"] {
            assert!(everything.contains(name), "seat {seat} sees {name}");
        }
    }
}
