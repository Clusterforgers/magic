//! Goal: `Pred::ArrivedFrom` holds for an object only while it lies in the zone it entered this turn from a zone of that kind, counts its latest move only, ignores a token made from nowhere, and the next turn clears it, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// Seat 0 holds Bear in hand and Elk in the library's place (hand too); every seat has a Guest on the battlefield.
fn table(seats: usize) -> GameState {
    state(
        (0..seats)
            .map(|seat| {
                if seat == 0 {
                    vec![creature("Bear", 2, 2), creature("Elk", 2, 2)]
                } else {
                    Vec::new()
                }
            })
            .collect(),
        (0..seats)
            .map(|seat| vec![creature(&format!("Guest {seat}"), 1, 1)])
            .collect(),
    )
}

fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn moved(state: &mut GameState, name: &str, from: Zone, to: Zone) {
    let event = Event::ZoneChanged {
        object: id(state, name),
        from,
        to,
        cause: ZoneCause::Moved,
    };
    apply_event(state, &event).unwrap();
}

fn arrived(state: &GameState, name: &str, origin: ZoneKind) -> bool {
    let object = state.object(id(state, name)).unwrap().reference();
    let source = state.object(id(state, "Guest 0")).unwrap().reference();
    Scope::announcing(state, ME, source)
        .selects(&Selector(vec![Pred::ArrivedFrom(origin)]), object)
        .unwrap()
}

#[rstest]
#[case::from_the_hand(ZoneKind::Hand, true)]
#[case::not_from_the_library(ZoneKind::Library, false)]
#[case::not_from_the_graveyard(ZoneKind::Graveyard, false)]
#[case::not_from_the_battlefield(ZoneKind::Battlefield, false)]
fn a_card_put_onto_the_battlefield_arrived_from_where_it_was(
    #[values(2, 3, 4)] seats: usize,
    #[case] origin: ZoneKind,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    moved(&mut game, "Bear", Zone::Hand(ME), Zone::Battlefield);
    assert_eq!(arrived(&game, "Bear", origin), expected);
    assert!(
        !arrived(&game, "Guest 0", ZoneKind::Hand),
        "it was there when the turn began"
    );
    assert!(!arrived(&game, "Elk", ZoneKind::Hand), "never moved");
}

#[rstest]
fn only_the_latest_move_counts_and_the_card_must_still_lie_there(#[values(2, 3, 4)] seats: usize) {
    let mut game = table(seats);
    moved(&mut game, "Bear", Zone::Hand(ME), Zone::Battlefield);
    moved(&mut game, "Bear", Zone::Battlefield, Zone::Graveyard(ME));
    assert!(arrived(&game, "Bear", ZoneKind::Battlefield));
    assert!(
        !arrived(&game, "Bear", ZoneKind::Hand),
        "the hand was the first move of an object that is gone"
    );
    moved(&mut game, "Bear", Zone::Graveyard(ME), Zone::Hand(ME));
    assert!(arrived(&game, "Bear", ZoneKind::Graveyard));
    assert!(!arrived(&game, "Bear", ZoneKind::Battlefield));
}
