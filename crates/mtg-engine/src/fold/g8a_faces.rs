//! Goal: `Prepared` holds for a prepared permanent only, `DoubleFaced` for a transforming or modal double-faced card in any zone (not a card of another layout) and `Transformed` for a transforming permanent with its back face up only, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_cards::definition::Layout;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;
use std::sync::Arc;

use super::apply_event;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;

const ME: PlayerId = PlayerId(0);

/// A creature card of `layout` whose second face is another creature.
fn faced(name: &str, layout: Layout) -> Arc<CardDefinition> {
    let front = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    let back = CardDefinition::new(format!("{name} Back"))
        .with_type(CardType::Creature)
        .with_power_toughness(4, 4);
    Arc::new(front.clone().with_faces(layout, vec![front, back]))
}

/// Seat 0 holds a transforming card in hand and controls Guest, a transforming Wolf and Elk, a modal Owl, a prepare Fox,
/// a flip Moth and a plain Bear; every other seat a Guest of its own.
fn table(seats: usize) -> GameState {
    let hand = |seat: usize| {
        if seat == 0 {
            vec![faced("Held", Layout::Transform)]
        } else {
            Vec::new()
        }
    };
    let field = |seat: usize| {
        let mut cards = vec![creature(&format!("Guest {seat}"), 2, 2)];
        if seat == 0 {
            cards.extend([
                faced("Wolf", Layout::Transform),
                faced("Elk", Layout::Transform),
                faced("Owl", Layout::ModalDfc),
                faced("Fox", Layout::Prepare),
                faced("Moth", Layout::Flip),
                creature("Bear", 2, 2),
            ]);
        }
        cards
    };
    let mut game = state(
        (0..seats).map(hand).collect(),
        (0..seats).map(field).collect(),
    );
    for event in [
        Event::Transformed {
            object: id(&game, "Wolf"),
            face: Some(1),
        },
        Event::Prepared {
            object: id(&game, "Fox"),
        },
    ] {
        apply_event(&mut game, &event).unwrap();
    }
    game
}

fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.whole_card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn holds(state: &GameState, pred: &Pred, name: &str) -> bool {
    let object = state.object(id(state, name)).unwrap().reference();
    let source = state.object(id(state, "Guest 0")).unwrap().reference();
    Scope::announcing(state, ME, source)
        .selects(&Selector(vec![pred.clone()]), object)
        .unwrap()
}

#[rstest]
#[case::prepared(Pred::Prepared, &["Fox"])]
#[case::double_faced(Pred::DoubleFaced, &["Held", "Wolf", "Elk", "Owl"])]
#[case::transformed(Pred::Transformed, &["Wolf"])]
fn the_face_facts_hold_for_the_objects_they_name(
    #[case] pred: Pred,
    #[case] want: &[&str],
    #[values(2, 3, 4)] seats: usize,
) {
    let game = table(seats);
    let names = [
        "Held", "Wolf", "Elk", "Owl", "Fox", "Moth", "Bear", "Guest 0", "Guest 1",
    ];
    let found: Vec<&str> = names
        .into_iter()
        .filter(|name| holds(&game, &pred, name))
        .collect();
    assert_eq!(found, want);
}

#[rstest]
fn a_transformed_permanent_turned_back_is_no_longer_transformed(#[values(2, 3, 4)] seats: usize) {
    let mut game = table(seats);
    let wolf = id(&game, "Wolf");
    apply_event(
        &mut game,
        &Event::Transformed {
            object: wolf,
            face: None,
        },
    )
    .unwrap();
    assert!(!holds(&game, &Pred::Transformed, "Wolf"));
    assert!(holds(&game, &Pred::DoubleFaced, "Wolf"));
}
