//! Goal: a game has no cap on its seats: a lobby of any size hands out its seats one by one (the last open seat to the last visitor, then "full"), the host's removal and a seat's own deck reach any seat number, and a started table of 8 to 64 seats resumes from its log and plays on.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{Bot, drive, game_id, roomy, table_setup};
use mtg_core::ids::PlayerId;
use mtg_engine::replay;
use mtg_protocol::ClientMessage;
use mtg_server::registry::{
    ClaimError, Created, Entrance, GameOptions, SeatClaim, SeatRequest, Seated,
};
use mtg_server::{
    EventStore, GameId, MemoryStore, Registry, ServerConfig, Visibility, demo, resume_game_with,
    spawn_game_with,
};
use rstest::rstest;

fn requests(seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

/// A public lobby of `seats` seats of which only the host's is held.
async fn lobby(registry: &Registry, seats: usize) -> Created {
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(1),
        ..GameOptions::default()
    };
    registry
        .create_with(&requests(seats), options, None)
        .await
        .unwrap()
}

async fn join(registry: &Registry, game: GameId, name: &str) -> Result<Seated, ClaimError> {
    let claim = SeatClaim {
        name: name.to_owned(),
        user: None,
    };
    registry.claim(game, Entrance::Code, claim).await
}

#[rstest]
#[tokio::test]
async fn the_last_open_seat_of_a_lobby_of_any_size_goes_to_the_last_visitor(
    #[values(2, 3, 4, 8, 12, 13, 24, 64)] seats: usize,
) {
    let registry = Registry::new(&ServerConfig::default());
    let created = lobby(&registry, seats).await;
    assert_eq!(created.seats.len(), 1, "only the host's seat is held");
    let game = created.id;

    for visitor in 1..seats {
        let seated = join(&registry, game, &format!("Visitor {visitor}"))
            .await
            .unwrap();
        assert_eq!(seated.seat, PlayerId::from_index(visitor), "in seat order");
        assert!(seated.token.is_some());
    }
    assert_eq!(
        join(&registry, game, "One too many").await.unwrap_err(),
        ClaimError::Full
    );
    let described = registry.describe(game).await.unwrap();
    assert_eq!(described.seats.len(), seats);
}

#[rstest]
#[tokio::test]
async fn a_released_seat_of_any_number_is_open_again_and_taken_by_the_next_visitor(
    #[values(13, 24, 64)] seats: usize,
) {
    let registry = Registry::new(&ServerConfig::default());
    let created = lobby(&registry, seats).await;
    let game = created.id;
    for visitor in 1..seats {
        join(&registry, game, &format!("Visitor {visitor}"))
            .await
            .unwrap();
    }
    let last = PlayerId::from_index(seats - 1);
    registry
        .release(game, last, mtg_server::registry::HOST)
        .await
        .unwrap();
    let again = join(&registry, game, "Latecomer").await.unwrap();
    assert_eq!(again.seat, last);
}

#[rstest]
#[tokio::test]
async fn a_table_of_many_seats_resumes_from_its_log_and_plays_on(
    #[values(8, 13, 24, 64)] players: usize,
) {
    let seed = 5;
    let id = game_id(1);
    let setup = table_setup(players, seed);
    let store = MemoryStore::new();
    let first = spawn_game_with(id, setup.clone(), store.clone(), roomy()).unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        bots.push(Bot::join(&first, n, seed).await);
    }
    for bot in &bots {
        first.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &first, 150).await, None);
    let stopped = first.status().await.unwrap();
    assert_eq!(stopped.events, store.load(id).await.unwrap().len() as u64);
    drop(first);

    let second = resume_game_with(store.clone(), id, setup.clone(), roomy())
        .await
        .unwrap();
    let resumed = second.status().await.unwrap();
    assert_eq!(resumed.events, stopped.events);
    assert_eq!(resumed.state_hash, stopped.state_hash);
    assert_eq!(resumed.waiting_on, stopped.waiting_on);
    for bot in &mut bots {
        bot.rejoin(&second).await;
    }
    assert_eq!(drive(&mut bots, &second, 150).await, None);

    let logged = store.load(id).await.unwrap();
    let status = second.status().await.unwrap();
    assert_eq!(
        status.state_hash,
        Some(replay(&setup, &logged).unwrap().state_hash()),
        "the log the table wrote replays to the table's own state"
    );
    for bot in &bots {
        assert_eq!(bot.errors, [], "{:?}", bot.seat);
    }
}
