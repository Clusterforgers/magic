//! Goal: `Ref::ActivePlayer` names the player whose turn it is, so `ControlledBy(ActivePlayer)` selects the permanents of that player only and follows the turn from seat to seat, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;

fn table(seats: usize) -> GameState {
    state(
        vec![Vec::new(); seats],
        (0..seats)
            .map(|seat| vec![creature(&format!("Seat {seat}"), 2, 2)])
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

/// The seats whose creature `ControlledBy(ActivePlayer)` selects.
fn selected(state: &GameState, seats: usize) -> Vec<usize> {
    let source = state.object(id(state, "Seat 0")).unwrap().reference();
    let scope = Scope::announcing(state, PlayerId(0), source);
    let filter = Selector(vec![Pred::ControlledBy(Ref::ActivePlayer)]);
    (0..seats)
        .filter(|seat| {
            let object = state
                .object(id(state, &format!("Seat {seat}")))
                .unwrap()
                .reference();
            scope.selects(&filter, object).unwrap()
        })
        .collect()
}

#[rstest]
fn the_active_player_is_whoever_has_the_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] turn: usize,
) {
    let mut game = table(seats);
    assert_eq!(selected(&game, seats), vec![0]);
    let began = Event::TurnBegan {
        number: 2,
        active: PlayerId(u64::try_from(turn).unwrap()),
        extra: false,
    };
    apply_event(&mut game, &began).unwrap();
    assert_eq!(selected(&game, seats), vec![turn]);
}
