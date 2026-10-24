//! Goal: `Pred::EnteredThisTurn` selects exactly the permanents that came onto the battlefield during the turn (a card put there, a token made, a permanent that left and returned), never one that was there when the turn began or one that has left since, and the next turn clears the record, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::tokens::TokenCreated;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);
const TOKEN: ObjectId = ObjectId(900);

/// `seats` players; seat 0 has an Old creature on the battlefield and a Newcomer in hand, every other seat a Guest.
fn table(seats: usize) -> GameState {
    let hand = |seat: usize| {
        if seat == 0 {
            vec![creature("Newcomer", 1, 1)]
        } else {
            Vec::new()
        }
    };
    let field = |seat: usize| {
        if seat == 0 {
            vec![creature("Old", 2, 2)]
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

fn fold(state: &GameState, events: &[Event]) -> GameState {
    let mut next = state.clone();
    for event in events {
        apply_event(&mut next, event).unwrap();
    }
    next
}

fn enters(state: &GameState, name: &str, from: Zone) -> Event {
    Event::ZoneChanged {
        object: id(state, name),
        from,
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    }
}

fn token(controller: PlayerId) -> Event {
    Event::TokenCreated(TokenCreated {
        object: TOKEN,
        copy_of: None,
        controller,
        name: "Spirit".into(),
        tapped: false,
        attacking: None,
        definition: Arc::new(CardDefinition::new("Spirit").with_type(CardType::Creature)),
    })
}

/// Whether the object `which` is selected by `EnteredThisTurn`.
fn entered(state: &GameState, which: ObjectId) -> bool {
    let object = state.object(which).unwrap().reference();
    Scope::announcing(
        state,
        ME,
        state.object(id(state, "Old")).unwrap().reference(),
    )
    .selects(&Selector(vec![Pred::EnteredThisTurn]), object)
    .unwrap()
}

#[rstest]
fn a_card_put_onto_the_battlefield_entered_and_the_ones_already_there_did_not(
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats);
    assert!(!entered(&pre, id(&pre, "Newcomer")), "still in the hand");
    let now = fold(&pre, &[enters(&pre, "Newcomer", Zone::Hand(ME))]);
    assert!(entered(&now, id(&now, "Newcomer")));
    assert!(!entered(&now, id(&now, "Old")));
    for seat in 1..seats {
        assert!(!entered(&now, id(&now, &format!("Guest {seat}"))));
    }
}

#[rstest]
fn a_token_made_this_turn_entered_under_any_control(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] controller: u64,
) {
    let pre = table(seats);
    let now = fold(&pre, &[token(PlayerId(controller))]);
    assert!(entered(&now, TOKEN));
    assert!(!entered(&now, id(&now, "Old")));
}

#[rstest]
fn a_permanent_that_left_is_a_new_object_and_one_that_came_back_entered_again(
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats);
    let old = id(&pre, "Old");
    let leave = Event::ZoneChanged {
        object: old,
        from: Zone::Battlefield,
        to: Zone::Hand(ME),
        cause: ZoneCause::Moved,
    };
    let gone = fold(&pre, std::slice::from_ref(&leave));
    assert!(!entered(&gone, old), "in the hand");
    let back = fold(&gone, &[enters(&gone, "Old", Zone::Hand(ME))]);
    assert!(entered(&back, old), "a new object that came in this turn");
    let again = fold(&back, &[leave]);
    assert!(!entered(&again, old), "left again");
}

#[rstest]
fn the_next_turn_clears_the_record_and_the_hash_follows(#[values(2, 3, 4)] seats: usize) {
    let pre = table(seats);
    let now = fold(
        &pre,
        &[enters(&pre, "Newcomer", Zone::Hand(ME)), token(PlayerId(1))],
    );
    assert_ne!(now.state_hash(), pre.state_hash(), "the record is hashed");
    let turn = Event::TurnBegan {
        number: 2,
        active: PlayerId(1),
        extra: false,
    };
    let next = fold(&now, std::slice::from_ref(&turn));
    assert!(!entered(&next, id(&next, "Newcomer")));
    assert!(!entered(&next, TOKEN));
}
