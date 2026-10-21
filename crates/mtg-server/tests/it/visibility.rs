//! Goal: a game is invite-only unless its creator asks for it to be listed, the registry and the public facts of the game agree on it, and a game made with open seats gives tokens only to the seats it holds, leaves the others nameless and shows them as open in the lobby.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Client, seat};
use mtg_protocol::{LobbySeat, SeatDeck, ServerMessage};
use mtg_server::registry::{GameOptions, SeatRequest};
use mtg_server::{GameId, Registry, SeatState, ServerConfig, Visibility, demo};
use rstest::rstest;
use uuid::Uuid;

fn requests(seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

fn registry() -> Registry {
    Registry::new(&ServerConfig::default())
}

#[tokio::test]
async fn a_game_is_invite_only_whichever_way_the_old_calls_make_it() {
    let registry = registry();
    let by_create = registry.create(&requests(2)).await.unwrap();
    let by_create_for = registry.create_for(&requests(2), None).await.unwrap();
    let registered = registry
        .register(common::table_setup(2, 1), Vec::new())
        .await
        .unwrap();
    for id in [by_create.id, by_create_for.id, registered.id] {
        assert_eq!(registry.visibility(id), Some(Visibility::Invite));
        let public = registry.describe(id).await.unwrap();
        assert_eq!(public.visibility, Visibility::Invite);
        assert!(
            public
                .seats
                .iter()
                .all(|seat| seat.state == SeatState::Issued)
        );
    }
    assert_eq!(
        by_create.seats.len(),
        2,
        "every seat has its token, as ever"
    );
    assert_eq!(Visibility::default(), Visibility::Invite);
    assert_eq!(
        GameOptions::default(),
        GameOptions {
            visibility: Visibility::Invite,
            held: None
        }
    );
}

#[rstest]
#[case::listed(Visibility::Public)]
#[case::invite_only(Visibility::Invite)]
#[tokio::test]
async fn a_creator_chooses_the_visibility(#[case] visibility: Visibility) {
    let registry = registry();
    let options = GameOptions {
        visibility,
        held: None,
    };
    let created = registry
        .create_with(&requests(3), options, None)
        .await
        .unwrap();
    assert_eq!(registry.visibility(created.id), Some(visibility));
    assert_eq!(
        registry.describe(created.id).await.unwrap().visibility,
        visibility
    );
    assert_eq!(created.seats.len(), 3);
}

#[tokio::test]
async fn an_unknown_game_has_no_visibility() {
    let nowhere = GameId(Uuid::from_u128(5));
    assert_eq!(registry().visibility(nowhere), None);
}

/// The seats a game of `seats` seats made with `held` holds from the start.
fn held_seats(seats: usize, held: Option<usize>) -> usize {
    held.map_or(seats, |held| held.max(1).min(seats))
}

#[rstest]
#[case::all_held(3, None)]
#[case::host_only(3, Some(1))]
#[case::two_of_four(4, Some(2))]
#[case::none_asked_for_still_holds_the_host(3, Some(0))]
#[case::more_than_there_are(2, Some(9))]
#[case::exactly_all(2, Some(2))]
#[tokio::test]
async fn only_the_held_seats_get_tokens_and_the_others_are_open(
    #[case] seats: usize,
    #[case] held: Option<usize>,
) {
    let registry = registry();
    let options = GameOptions {
        visibility: Visibility::Public,
        held,
    };
    let created = registry
        .create_with(&requests(seats), options, None)
        .await
        .unwrap();
    let held = held_seats(seats, held);

    // Tokens exist for the held seats, in seat order, and open exactly their own seats.
    assert_eq!(created.seats.len(), held);
    for (index, (number, token)) in created.seats.iter().enumerate() {
        assert_eq!(number.index(), index);
        let access = registry.authenticate(created.id, token.expose()).unwrap();
        assert_eq!(access.seat, *number);
    }

    // The public facts say which seats are open, and an open seat has no name.
    let public = registry.describe(created.id).await.unwrap();
    assert_eq!(public.seats.len(), seats);
    for (index, summary) in public.seats.iter().enumerate() {
        assert_eq!(summary.seat, seat(index));
        if index >= held {
            assert_eq!(summary.state, SeatState::Open, "seat {index}");
            assert_eq!(summary.name, "");
        } else {
            assert_eq!(summary.state, SeatState::Issued, "seat {index}");
            assert_eq!(summary.name, format!("Player {index}"));
        }
    }
}

#[tokio::test]
async fn the_lobby_shows_an_open_seat_with_no_name_and_counts_it_as_needed() {
    let registry = registry();
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(1),
    };
    let created = registry
        .create_with(&requests(3), options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();

    let mut host = Client::join(&handle, 0).await;
    assert!(matches!(host.next().await, ServerMessage::Welcome { .. }));
    let ServerMessage::Lobby { seats, needed } = host.next().await else {
        panic!("the host is sent the roster");
    };
    let open = |number: usize| LobbySeat {
        seat: seat(number),
        name: String::new(),
        connected: false,
        ready: false,
        deck: None,
    };
    assert_eq!(
        seats,
        [
            LobbySeat {
                seat: seat(0),
                name: "Player 0".to_owned(),
                connected: true,
                ready: false,
                deck: Some(SeatDeck {
                    name: "Mono Green Stompers".to_owned(),
                    format: "normal".to_owned(),
                    manual_cards: 0,
                }),
            },
            open(1),
            open(2),
        ]
    );
    assert_eq!(needed, 3, "an open seat has still to press Ready");
}

#[tokio::test]
async fn a_game_cannot_start_while_a_seat_is_open() {
    let registry = registry();
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(1),
    };
    let created = registry
        .create_with(&requests(2), options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let host = Client::join(&handle, 0).await;
    host.say(&handle, mtg_protocol::ClientMessage::Ready).await;
    assert!(!handle.status().await.unwrap().started);
}
