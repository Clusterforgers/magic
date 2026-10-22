//! Goal: a copy of a spell on the stack is shown to every seat as a spell with the name, types, targets and the marker of the original and no more, at tables of two, three and four seats: the hand of the player who copied it stays hidden from the other seats exactly as before, the events that make the copy (the copy of a spell, of an ability, the end of a copy) are public and add nothing the stack did not show, and the wire form of an object that is no copy is unchanged.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{CopyView, EventView, ObjectView, StateView, project_events};
use mtg_testkit::cards::{island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn view_of(state: &GameState, viewer: usize, id: ObjectId) -> Option<ObjectView> {
    StateView::for_player(state, PlayerId::from_index(viewer))
        .objects
        .into_iter()
        .find(|o| o.id == id)
}

/// Seat 0 has cast Lightning Bolt at seat 1 and Twincast at the Bolt; the copy is on the stack above both, and the hand of
/// seat 0 holds a card nobody else may see.
fn with_a_copy_on_the_stack(seats: usize) -> (Driver, ObjectId, ObjectId, GameState, Vec<Event>) {
    let mut lands: Vec<_> = (0..2).map(|_| ready(island())).collect();
    lands.push(ready(mountain()));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(
                0,
                [
                    card("Lightning Bolt").unwrap(),
                    card("Twincast").unwrap(),
                    card("Brainstorm").unwrap(),
                ],
            )
            .battlefield(0, lands),
    )
    .unwrap();
    let foe = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[foe]).unwrap();
    let bolt = *driver.state().stack().last().unwrap();
    let target = Target::Object(driver.state().object(bolt).unwrap().reference());
    driver.begin_cast("Twincast").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Island", "Island"], 0).unwrap();
    let before_state = driver.state().clone();
    let before = driver.engine().events().len();
    for _ in 0..seats {
        if !matches!(
            driver.engine().decision().map(|d| &d.kind),
            Some(DecisionKind::Priority(_))
        ) {
            break;
        }
        driver.pass().unwrap();
    }
    driver
        .answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(false)))
        .unwrap();
    let events = driver.engine().events()[before..].to_vec();
    let copy = *driver.state().stack().last().unwrap();
    assert!(driver.state().object(copy).unwrap().is_spell_copy());
    (driver, bolt, copy, before_state, events)
}

#[rstest]
fn a_copy_of_a_spell_is_shown_to_every_seat_with_the_targets_of_the_original(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] viewer: usize,
) {
    let (driver, bolt, copy, ..) = with_a_copy_on_the_stack(seats);
    let shown = view_of(driver.state(), viewer, copy).unwrap();
    let original = view_of(driver.state(), viewer, bolt).unwrap();
    assert_eq!(shown.name, "Lightning Bolt");
    assert_eq!(shown.zone, Zone::Stack);
    assert_eq!(shown.kind, ObjectKind::Token);
    assert_eq!(shown.controller, PlayerId(0));
    assert_eq!(shown.copy, Some(CopyView { of: Some(bolt) }));
    assert_eq!(shown.targets, original.targets);
    assert_eq!(shown.types, original.types);
    assert_eq!(shown.abilities, original.abilities);
    assert_eq!(original.copy, None);
    assert_eq!(original.kind, ObjectKind::Card);
}

#[rstest]
fn the_hand_of_the_player_who_copied_stays_hidden(#[values(2, 3, 4)] seats: usize) {
    let (driver, ..) = with_a_copy_on_the_stack(seats);
    let hand = driver.state().zone(Zone::Hand(PlayerId(0))).to_vec();
    assert_eq!(hand.len(), 1);
    assert!(view_of(driver.state(), 0, hand[0]).is_some());
    for viewer in 1..seats {
        assert!(
            view_of(driver.state(), viewer, hand[0]).is_none(),
            "seat {viewer} does not see the card"
        );
    }
}

#[rstest]
fn the_events_of_a_copy_are_public(#[values(2, 3, 4)] seats: usize) {
    let (_, _, copy, before, events) = with_a_copy_on_the_stack(seats);
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(&before, &events, &viewers, 0).unwrap();
    for (viewer, batch) in batches.iter().enumerate() {
        let seen = batch.events.iter().find_map(|view| match view {
            EventView::Public(Event::SpellCopied(made)) => Some(made.object),
            _ => None,
        });
        assert_eq!(seen, Some(copy), "seat {viewer} sees the copy being made");
        assert!(
            batch.objects.iter().any(|o| o.id == copy),
            "seat {viewer} is sent the copy"
        );
    }
}

#[test]
fn the_wire_form_of_a_copy_names_the_original_and_an_object_that_is_no_copy_has_no_such_field() {
    let (driver, bolt, copy, ..) = with_a_copy_on_the_stack(2);
    let original = serde_json::to_value(view_of(driver.state(), 1, bolt).unwrap()).unwrap();
    assert!(original.get("copy").is_none());
    let shown = serde_json::to_value(view_of(driver.state(), 1, copy).unwrap()).unwrap();
    assert_eq!(shown["copy"], serde_json::json!({ "of": bolt.get() }));
    assert_eq!(shown["kind"], "Token");
}
