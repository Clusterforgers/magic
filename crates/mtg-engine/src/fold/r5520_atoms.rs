//! Goal: `Foretold` holds exactly for a card face down in exile as foretold, and `HasName` for every object but a face-down one (CR 708.2a), at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::{Pred, Selector};
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::object::FaceDown;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// Seat 0 holds Told and Kept in hand and Bear and Mask on the battlefield; every seat a Guest.
fn table(seats: usize) -> GameState {
    let hand = |seat: usize| {
        if seat == 0 {
            vec![creature("Told", 1, 1), creature("Kept", 1, 1)]
        } else {
            Vec::new()
        }
    };
    let field = |seat: usize| {
        let mut cards = vec![creature(&format!("Guest {seat}"), 2, 2)];
        if seat == 0 {
            cards.extend([creature("Bear", 2, 2), creature("Mask", 3, 3)]);
        }
        cards
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

fn holds(state: &GameState, pred: Pred, name: &str) -> bool {
    holds_for(state, pred, id(state, name))
}

/// Whether the predicate holds for the object, found by id (a face-down card shows no name).
fn holds_for(state: &GameState, pred: Pred, object: ObjectId) -> bool {
    let object = state.object(object).unwrap().reference();
    let source = state.object(id(state, "Guest 0")).unwrap().reference();
    Scope::announcing(state, ME, source)
        .selects(&Selector(vec![pred]), object)
        .unwrap()
}

/// Moves the card to exile and lays it there as `face`.
fn exile(game: &mut GameState, name: &str, face: Option<FaceDown>) {
    let object = id(game, name);
    let event = Event::ZoneChanged {
        object,
        from: Zone::Hand(ME),
        to: Zone::Exile,
        cause: ZoneCause::Moved,
    };
    apply_event(game, &event).unwrap();
    game.objects.get_mut(&object).unwrap().face_down = face;
}

#[rstest]
#[case::foretold(Some(FaceDown::Foretold { turn: 1 }), true)]
#[case::face_up(None, false)]
#[case::held(Some(FaceDown::Held { by: crate::object::ObjectRef { id: ObjectId(1), counter: 0 }, looker: ME }), false)]
fn foretold_is_a_card_foretold_in_exile(
    #[values(2, 3, 4)] seats: usize,
    #[case] face: Option<FaceDown>,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    exile(&mut game, "Told", face);
    assert_eq!(holds(&game, Pred::Foretold, "Told"), expected);
    assert!(
        !holds(&game, Pred::Foretold, "Kept"),
        "a card in hand is not foretold"
    );
    assert!(!holds(&game, Pred::Foretold, "Bear"));
}

#[rstest]
fn every_object_but_a_face_down_one_has_a_name(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] masked: bool,
) {
    let mut game = table(seats);
    let mask = id(&game, "Mask");
    if masked {
        game.objects.get_mut(&mask).unwrap().face_down = Some(FaceDown::Morph);
    }
    assert_eq!(holds_for(&game, Pred::HasName, mask), !masked);
    assert_eq!(
        holds_for(&game, Pred::Not(Box::new(Pred::HasName)), mask),
        masked
    );
    for named in ["Bear", "Told", "Guest 0"] {
        assert!(holds(&game, Pred::HasName, named), "{named} has a name");
    }
    let last = format!("Guest {}", seats - 1);
    assert!(holds(&game, Pred::HasName, &last));
}
