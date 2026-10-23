//! Goal: phasing is public at tables of two, three and four seats: the events of a permanent phasing out and in reach every seat as logged, a phased-out permanent and what phased out with it are listed in `phased_out` and not in `battlefield` for every viewer and carry the zone `PhasedOut`, the field is left out of the JSON while nothing is phased out, and a view advanced by the batches equals the view of the state after them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, apply_event, phasing};
use mtg_protocol::{EventView, StateView, project_events};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Scenario, attached_to, ready};
use rstest::rstest;

/// Seat 0 has a Plains, a Bear and a Blade on the Bear; every other seat has a Wolf. Returns the state and the ids of
/// the Bear and the Blade.
fn table(seats: usize) -> (GameState, ObjectId, ObjectId) {
    let blade = std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Blade")
            .with_type(mtg_core::card::CardType::Artifact)
            .with_subtype("Equipment"),
    );
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(plains()),
            ready(free_creature("Bear", 2, 2)),
            attached_to(ready(blade), 0, 1),
        ],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Wolf", 3, 3))]);
    }
    let state = GameState::from_setup(&scenario.setup().unwrap()).unwrap();
    let id = |name: &str| {
        state
            .objects()
            .find(|o| o.card().name == name)
            .unwrap()
            .id()
    };
    let (bear, blade) = (id("Bear"), id("Blade"));
    (state, bear, blade)
}

fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..seats).map(PlayerId::from_index).collect()
}

#[rstest]
fn phasing_out_and_in_is_public_and_the_views_follow(#[values(2, 3, 4)] seats: usize) {
    let (before, bear, blade) = table(seats);
    let seats_list = viewers(seats);
    let out = phasing::out_events(&before, &[bear]);
    assert_eq!(out.len(), 2, "the Bear and the Blade attached to it");
    let batches = project_events(&before, &out, &seats_list, 0).unwrap();
    let mut after = before.clone();
    for event in &out {
        apply_event(&mut after, event).unwrap();
    }
    for (viewer, batch) in seats_list.iter().zip(&batches) {
        assert!(
            batch
                .events
                .iter()
                .all(|event| matches!(event, EventView::Public(_)))
        );
        let view = StateView::for_player(&after, *viewer);
        assert_eq!(view.table.phased_out, vec![bear, blade]);
        let on_board = view.table.battlefield.cards.clone().unwrap();
        assert!(!on_board.contains(&bear) && !on_board.contains(&blade));
        for id in [bear, blade] {
            let object = view.objects.iter().find(|o| o.id == id).unwrap();
            assert_eq!(object.zone, Zone::PhasedOut);
            assert!(object.permanent.is_some());
        }
        let mut advanced = StateView::for_player(&before, *viewer);
        advanced.apply(batch);
        assert_eq!(advanced, view, "seat {viewer:?}");
    }
    // Phasing in: the Bear and the Blade are back on the battlefield list for every seat.
    let back = phasing::in_events(&after, PlayerId(0));
    assert_eq!(back.len(), 2);
    let batches = project_events(&after, &back, &seats_list, 0).unwrap();
    let mut finally = after.clone();
    for event in &back {
        apply_event(&mut finally, event).unwrap();
    }
    for (viewer, batch) in seats_list.iter().zip(&batches) {
        assert!(
            batch
                .events
                .iter()
                .all(|event| matches!(event, EventView::Public(Event::PermanentPhasedIn { .. })))
        );
        let view = StateView::for_player(&finally, *viewer);
        assert_eq!(view.table.phased_out, Vec::<ObjectId>::new());
        let mut advanced = StateView::for_player(&after, *viewer);
        advanced.apply(batch);
        assert_eq!(advanced, view);
    }
}

#[rstest]
fn the_field_is_left_out_of_the_json_while_nothing_is_phased_out(#[values(2, 3, 4)] seats: usize) {
    let (state, bear, _) = table(seats);
    let json = serde_json::to_value(StateView::for_player(&state, PlayerId(0)).table).unwrap();
    assert!(json.get("phased_out").is_none());
    let mut state = state;
    for event in phasing::out_events(&state.clone(), &[bear]) {
        apply_event(&mut state, &event).unwrap();
    }
    let json = serde_json::to_value(StateView::for_player(&state, PlayerId(0)).table).unwrap();
    assert_eq!(json["phased_out"].as_array().unwrap().len(), 2);
}
