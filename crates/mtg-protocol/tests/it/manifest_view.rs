//! Goal: a manifested or cloaked card is a 2/2 on the battlefield for every seat to see and only its controller sees which card it is, wherever the library it came from stood: the move is whole for the controller and hidden for the others (the owner of a card taken from their library by another player included), the libraries show counts, the live view and the replay agree, a cloaked card shows its ward to every seat, and no part of what a seat is told in whole games names a face-down card it may not look at; at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::events::ZoneCause;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState, replay};
use mtg_protocol::{EventBatch, EventView, HiddenEvent, ObjectView, StateView, project_events};
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario, corpus, manifest_fuzz_setup, ready};
use rstest::rstest;
use serde_json::Value;
use std::sync::Arc;

use crate::common::{run_game, with_unique_names};

const CARD: &str = "Serra Angel";

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Every viewer of a table of `seats`: each seat and one spectator.
fn viewers(seats: usize) -> Vec<PlayerId> {
    (0..=seats).map(PlayerId::from_index).collect()
}

/// A library with `top` over plain cards that are named for their seat.
fn library(owner: usize, top: Arc<CardDefinition>) -> Vec<Arc<CardDefinition>> {
    let mut cards = vec![top];
    cards.extend((0..6).map(|i| free_creature(&format!("Pad {owner} {i}"), 1, 1)));
    cards
}

/// Seat 0 controls Mastery of the Unseen and nine Plains; the top card of seat `owner`'s library is `CARD`.
fn table(seats: usize, owner: usize) -> Scenario {
    let mut battlefield = vec![ready(corpus::card("Mastery of the Unseen").unwrap())];
    battlefield.extend((0..9).map(|_| ready(plains())));
    (0..seats).fold(
        Scenario::new(seats).battlefield(0, battlefield),
        |scenario, seat| {
            let top = if seat == owner {
                corpus::card(CARD).unwrap()
            } else {
                free_creature(&format!("Top {seat}"), 1, 1)
            };
            scenario.library(seat, library(seat, top))
        },
    )
}

/// Whether the text of `value` holds `name` anywhere.
fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

/// Whether any part of the batch holds the card's name.
fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

fn object_of(batch: &EventBatch, id: ObjectId) -> Option<&ObjectView> {
    batch.objects.iter().find(|object| object.id == id)
}

/// The moves of `id` to the battlefield that the batch tells, whole or hidden.
fn arrivals(batch: &EventBatch, id: ObjectId) -> (usize, usize) {
    let whole = batch
        .events
        .iter()
        .filter(|view| {
            matches!(view, EventView::Public(Event::ZoneChanged { object, to: Zone::Battlefield, .. }) if *object == id)
        })
        .count();
    let hidden = batch
        .events
        .iter()
        .filter(|view| {
            matches!(
                view,
                EventView::Hidden(HiddenEvent::ZoneChanged {
                    to: Zone::Battlefield,
                    ..
                })
            )
        })
        .count();
    (whole, hidden)
}

/// Seat 0 activates Mastery of the Unseen and lets it resolve: the events, the state before them and the new permanent.
fn manifest_by_activation(seats: usize) -> (GameState, Vec<Event>, GameState, ObjectId) {
    let scenario = table(seats, 0);
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Mastery of the Unseen").unwrap();
    driver
        .answer(mtg_engine::decision::Answer::PayMana(
            mtg_engine::decision::PayChoice::Auto,
        ))
        .unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let id = driver
        .state()
        .objects()
        .find(|object| object.face_down().is_some())
        .map(mtg_engine::object::GameObject::id)
        .unwrap();
    (
        before,
        driver.engine().events().to_vec(),
        driver.state().clone(),
        id,
    )
}

#[rstest]
fn only_the_controller_sees_which_card_was_manifested(#[values(2, 3, 4)] seats: usize) {
    let (before, events, _, id) = manifest_by_activation(seats);
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let owner = *viewer == seat(0);
        let object = object_of(batch, id).expect("every viewer sees the permanent");
        assert!(object.face_down, "it lies face down for {viewer:?}");
        assert_eq!(object.zone, Zone::Battlefield);
        assert_eq!(object.types, [CardType::Creature]);
        assert_eq!((object.power, object.toughness), (Some(2), Some(2)));
        assert_eq!(object.name == CARD, owner, "the name: {viewer:?}");
        assert_eq!(batch_names(batch, CARD), owner, "who is told: {viewer:?}");
        // The move is told whole to the controller and hidden to the rest, with its cause.
        assert_eq!(arrivals(batch, id), if owner { (1, 0) } else { (0, 1) });
    }
}

#[rstest]
fn libraries_show_counts_and_the_views_of_a_replay_agree(#[values(2, 3, 4)] seats: usize) {
    let (before, events, after, id) = manifest_by_activation(seats);
    let scenario = table(seats, 0);
    let again = replay(&scenario.setup().unwrap(), &events).unwrap();
    for viewer in viewers(seats) {
        for state in [&after, &again] {
            let view = StateView::for_player(state, viewer);
            let mine = view.table.players[0].library.clone();
            assert_eq!(mine.count, 6, "seven cards, one manifested");
            assert_eq!(mine.cards, None, "a library lists no cards");
            assert_eq!(
                view.objects
                    .iter()
                    .find(|object| object.id == id)
                    .map(|object| object.name == CARD),
                Some(viewer == seat(0))
            );
            assert_eq!(
                names(&serde_json::to_value(&view).unwrap(), CARD),
                viewer == seat(0)
            );
        }
    }
    drop(before);
}

/// The cards of a library that another player manifests: seat 0 takes the top card of seat 1's library.
#[rstest]
#[case::manifested(ZoneCause::Manifested { by: seat(0) }, "")]
#[case::cloaked(ZoneCause::Cloaked { by: seat(0) }, "Ward")]
fn a_card_taken_from_another_library_is_told_to_its_new_controller_alone(
    #[case] cause: ZoneCause,
    #[case] ward: &str,
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = table(seats, 1);
    let before = GameState::from_setup(&scenario.setup().unwrap()).unwrap();
    let top = *before.zone(Zone::Library(seat(1))).last().unwrap();
    let events = [
        Event::ZoneChanged {
            object: top,
            from: Zone::Library(seat(1)),
            to: Zone::Battlefield,
            cause,
        },
        Event::Entered {
            object: top,
            tapped: false,
            controller: seat(0),
            chosen: None,
        },
    ];
    let all = viewers(seats);
    let batches = project_events(&before, &events, &all, 0).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let controller = *viewer == seat(0);
        let object = object_of(batch, top).expect("the permanent is there for everyone");
        assert!(object.face_down);
        assert_eq!(object.owner, seat(1));
        assert_eq!(object.controller, seat(0));
        assert_eq!(object.name == CARD, controller, "{viewer:?}");
        assert_eq!(
            batch_names(batch, CARD),
            controller,
            "the owner is not told either: {viewer:?}"
        );
        assert_eq!(
            arrivals(batch, top),
            if controller { (1, 0) } else { (0, 1) }
        );
        // The ward of a cloaked card is public.
        if !controller {
            assert_eq!(
                object.keywords,
                if ward.is_empty() {
                    Vec::<String>::new()
                } else {
                    vec![ward.to_owned()]
                }
            );
        }
    }
}

/// The names of the cards that lie face down in `state` and that `viewer` may not look at.
fn face_down_names(state: &GameState, viewer: PlayerId) -> Vec<String> {
    state
        .objects()
        .filter(|o| o.face_down().is_some() && !state.may_look_at(viewer, o.id()))
        .map(|o| o.whole_card().name.clone())
        .collect()
}

/// Games with the manifest cards under unique names: no part of what a seat is told names a card that lies face down after
/// the answer and that the seat may not look at.
#[rstest]
#[case(2, 51)]
#[case(3, 52)]
#[case(4, 53)]
fn no_face_down_card_reaches_a_seat_that_may_not_look_at_it(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let all = viewers(players);
    let (mut made, mut checked) = (0usize, 0usize);
    let setup = with_unique_names(manifest_fuzz_setup(players, seed));
    let engine = run_game(setup, seed, |step| {
        made += step
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::ZoneChanged {
                        cause: ZoneCause::Manifested { .. } | ZoneCause::Cloaked { .. },
                        ..
                    }
                )
            })
            .count();
        let batches = project_events(step.before, step.events, &all, step.from_seq).unwrap();
        for (viewer, batch) in all.iter().zip(&batches) {
            for name in face_down_names(step.after, *viewer) {
                checked += 1;
                assert!(
                    !batch_names(batch, &name),
                    "seed {seed} answer {} viewer {viewer:?}: the batch names {name}",
                    step.answers
                );
            }
            let view = StateView::for_player(step.after, *viewer);
            for name in face_down_names(step.after, *viewer) {
                assert!(
                    !names(&serde_json::to_value(&view).unwrap(), &name),
                    "seed {seed} answer {} viewer {viewer:?}: the view names {name}",
                    step.answers
                );
            }
        }
    });
    assert!(engine.result().is_some(), "the game finishes");
    assert!(
        made > 0 && checked > 0,
        "the test is not vacuous: {made} {checked}"
    );
}
