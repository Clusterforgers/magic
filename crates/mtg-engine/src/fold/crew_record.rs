//! Goal: a `Crewed` event records the creatures on the battlefield that were tapped to crew (or saddle) a permanent, `Pred::Crewed` and `Pred::Saddled` select exactly those creatures for the permanents their reference names (a crew never answers a saddle predicate and the other way round, another permanent's riders do not count, nor does a creature that was not on the battlefield or that changed zones since), the record is cleared by the next turn and is part of the state hash only while it is not empty, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// `seats` players; seat 0 controls two Vehicles (Skiff and Barge) and two riders (Ann and Bob), a Bystander is in
/// its hand, and every other seat controls a Guest.
fn table(seats: usize) -> GameState {
    let hand = |seat: usize| {
        if seat == 0 {
            vec![creature("Bystander", 1, 1)]
        } else {
            Vec::new()
        }
    };
    let field = |seat: usize| {
        if seat == 0 {
            vec![
                creature("Skiff", 4, 4),
                creature("Barge", 4, 4),
                creature("Ann", 1, 1),
                creature("Bob", 1, 1),
            ]
        } else {
            vec![creature(&format!("Guest {seat}"), 2, 2)]
        }
    };
    state(
        (0..seats).map(hand).collect(),
        (0..seats).map(field).collect(),
    )
}

fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn crewed(state: &GameState, vehicle: &str, riders: &[&str], saddle: bool) -> Event {
    Event::Crewed {
        object: id(state, vehicle),
        player: ME,
        creatures: riders.iter().map(|rider| id(state, rider)).collect(),
        saddle,
    }
}

fn fold(state: &GameState, events: &[Event]) -> GameState {
    let mut next = state.clone();
    for event in events {
        apply_event(&mut next, event).unwrap();
    }
    next
}

/// Whether `rider` is selected by `pred` for a source named `source`.
fn selected(state: &GameState, pred: Pred, source: &str, rider: &str) -> bool {
    let source = state.object(id(state, source)).unwrap().reference();
    let rider = state.object(id(state, rider)).unwrap().reference();
    Scope::announcing(state, ME, source)
        .selects(&Selector(vec![pred]), rider)
        .unwrap()
}

#[rstest]
#[case::the_rider_of_the_source("Skiff", "Ann", true)]
#[case::the_other_rider_did_not_pay("Skiff", "Bob", false)]
#[case::another_vehicle_has_no_riders("Barge", "Ann", false)]
#[case::the_vehicle_itself_did_not_pay("Skiff", "Skiff", false)]
fn crew_selects_the_creatures_tapped_for_the_source(
    #[case] source: &str,
    #[case] rider: &str,
    #[case] expected: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats);
    let now = fold(&pre, &[crewed(&pre, "Skiff", &["Ann"], false)]);
    assert_eq!(
        selected(&now, Pred::Crewed(Ref::This), source, rider),
        expected
    );
    // The saddle predicate never answers a crew.
    assert!(!selected(&now, Pred::Saddled(Ref::This), source, rider));
}

#[rstest]
#[case::saddled(true)]
#[case::crewed(false)]
fn a_saddle_and_a_crew_are_recorded_apart(#[case] saddle: bool, #[values(2, 3, 4)] seats: usize) {
    let pre = table(seats);
    let now = fold(&pre, &[crewed(&pre, "Skiff", &["Ann", "Bob"], saddle)]);
    for rider in ["Ann", "Bob"] {
        assert_eq!(
            selected(&now, Pred::Saddled(Ref::This), "Skiff", rider),
            saddle
        );
        assert_eq!(
            selected(&now, Pred::Crewed(Ref::This), "Skiff", rider),
            !saddle
        );
    }
}

#[rstest]
fn the_crews_of_a_turn_add_up_and_each_vehicle_keeps_its_own(#[values(2, 3, 4)] seats: usize) {
    let pre = table(seats);
    let now = fold(
        &pre,
        &[
            crewed(&pre, "Skiff", &["Ann"], false),
            crewed(&pre, "Barge", &["Bob"], false),
            crewed(&pre, "Skiff", &["Bob"], false),
        ],
    );
    assert!(selected(&now, Pred::Crewed(Ref::This), "Skiff", "Ann"));
    assert!(selected(&now, Pred::Crewed(Ref::This), "Skiff", "Bob"));
    assert!(!selected(&now, Pred::Crewed(Ref::This), "Barge", "Ann"));
    assert!(selected(&now, Pred::Crewed(Ref::This), "Barge", "Bob"));
}

#[rstest]
fn the_reference_names_the_permanent_whatever_the_source_is(#[values(2, 3, 4)] seats: usize) {
    let pre = table(seats);
    let now = fold(&pre, &[crewed(&pre, "Skiff", &["Ann"], false)]);
    let skiff = Ref::Each(Selector(vec![Pred::Other, Pred::Type(CardType::Creature)]));
    // Source Bob: every creature but Bob is "named", Skiff among them.
    assert!(selected(&now, Pred::Crewed(skiff), "Bob", "Ann"));
    let nobody = Ref::Each(Selector(vec![Pred::Not(Box::new(Pred::Type(
        CardType::Creature,
    )))]));
    assert!(!selected(&now, Pred::Crewed(nobody), "Bob", "Ann"));
}

#[rstest]
fn a_creature_that_was_not_on_the_battlefield_is_not_recorded(#[values(2, 3, 4)] seats: usize) {
    let pre = table(seats);
    let now = fold(&pre, &[crewed(&pre, "Skiff", &["Bystander"], false)]);
    assert!(!selected(
        &now,
        Pred::Crewed(Ref::This),
        "Skiff",
        "Bystander"
    ));
    assert_eq!(
        now.state_hash(),
        fold(&pre, &[crewed(&pre, "Skiff", &[], false)]).state_hash(),
        "nothing recorded either way"
    );
}

#[rstest]
fn a_rider_that_changed_zones_since_is_a_new_object_and_paid_for_nothing(
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats);
    let ann = id(&pre, "Ann");
    let crew = fold(&pre, &[crewed(&pre, "Skiff", &["Ann"], false)]);
    let leave = Event::ZoneChanged {
        object: ann,
        from: Zone::Battlefield,
        to: Zone::Hand(ME),
        cause: ZoneCause::Moved,
    };
    let back = Event::ZoneChanged {
        object: ann,
        from: Zone::Hand(ME),
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    };
    let returned = fold(&crew, &[leave, back]);
    assert!(!selected(
        &returned,
        Pred::Crewed(Ref::This),
        "Skiff",
        "Ann"
    ));
}

#[rstest]
fn the_next_turn_clears_the_record_and_the_hash_follows(#[values(2, 3, 4)] seats: usize) {
    let pre = table(seats);
    let crew = fold(&pre, &[crewed(&pre, "Skiff", &["Ann"], false)]);
    assert_ne!(crew.state_hash(), pre.state_hash(), "the record is hashed");
    let turn = Event::TurnBegan {
        number: 2,
        active: PlayerId(1),
        extra: false,
    };
    let next = fold(&crew, std::slice::from_ref(&turn));
    assert!(!selected(&next, Pred::Crewed(Ref::This), "Skiff", "Ann"));
    assert_eq!(
        next.state_hash(),
        fold(&pre, &[turn]).state_hash(),
        "an empty record hashes as no record"
    );
}
