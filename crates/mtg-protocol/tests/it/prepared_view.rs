//! Goal: that a permanent is prepared and the cast of a copy of its spell face are public to every seat (the designation in the events and in the permanent's view, the copy as the card of exile it is), while the other cards in the hand of the player who casts stay hidden from the other seats, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, apply_event};
use mtg_protocol::{EventView, StateView, project_events};
use mtg_testkit::cards::{forest, plains};
use mtg_testkit::prepare_cards::owl;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::assert_clean;

#[rstest]
fn a_prepared_permanent_and_the_cast_of_its_spell_face_are_public_and_the_hand_stays_hidden(
    #[values(2, 3, 4)] seats: usize,
) {
    let secret = mtg_testkit::corpus::card("Lightning Bolt").unwrap();
    let mut scenario = Scenario::new(seats)
        .hand(0, vec![owl(), secret])
        .battlefield(0, [ready(plains()), ready(plains())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let bolt = driver
        .state()
        .zone(Zone::Hand(PlayerId(0)))
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == "Lightning Bolt")
        .unwrap();
    driver.cast_tapping("Soothing Owl", &["Plains"]).unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    let owl = driver.id_of("Soothing Owl").unwrap();
    // The table shows the designation to every seat while the Owl is prepared.
    for viewer in (0..seats).map(PlayerId::from_index) {
        let view = StateView::for_player(driver.state(), viewer);
        let shown = view.objects.iter().find(|o| o.id == owl).unwrap();
        assert!(shown.permanent.as_ref().unwrap().prepared, "{viewer:?}");
    }
    driver.cast_prepared("Soothing Owl").unwrap();
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let events = driver.engine().events().to_vec();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    let hidden = BTreeMap::from([(bolt, "Lightning Bolt".to_owned())]);
    for (seat, batch) in batches.iter().enumerate() {
        for wanted in [
            Event::Prepared { object: owl },
            Event::Unprepared { object: owl },
        ] {
            assert!(
                batch
                    .events
                    .iter()
                    .any(|event| matches!(event, EventView::Public(e) if *e == wanted)),
                "seat {seat} is told {wanted:?}"
            );
        }
        assert!(
            batch.events.iter().any(|event| matches!(
                event,
                EventView::Public(Event::CardCopied(copy)) if copy.of.id == owl
            )),
            "seat {seat} sees the copy of the spell face"
        );
        if seat != 0 {
            assert_clean(batch, &hidden);
        }
    }
    // After the cast the permanent is not prepared, for every seat.
    for viewer in &viewers {
        let view = StateView::for_player(driver.state(), *viewer);
        let shown = view.objects.iter().find(|o| o.id == owl).unwrap();
        assert!(!shown.permanent.as_ref().unwrap().prepared, "{viewer:?}");
    }
    let mut state = driver.state().clone();
    apply_event(&mut state, &Event::Prepared { object: owl }).unwrap();
    assert!(state.object(owl).unwrap().permanent().unwrap().prepared());
}
