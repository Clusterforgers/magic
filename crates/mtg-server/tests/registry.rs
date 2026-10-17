//! Goal: the registry maps each token to exactly one seat of one game, never keeps a token in clear, enforces the cap, keeps shuffles server-random unless a test fixes the seed, and its janitor drops finished games, idle lobbies and abandoned games on schedule while leaving live ones alone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, Instant};

use common::seat;
use mtg_engine::error::SetupError;
use mtg_protocol::ClientMessage;
use mtg_server::config::Retention;
use mtg_server::registry::{CreateError, SeatRequest};
use mtg_server::{GameHandle, Registry, ServerConfig, StartError, demo};

fn requests(decks: &[&str]) -> Vec<SeatRequest> {
    decks
        .iter()
        .enumerate()
        .map(|(i, id)| SeatRequest {
            name: format!("Player {i}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

fn pair() -> Vec<SeatRequest> {
    requests(&["mono-green", "mono-red"])
}

async fn start(handle: &GameHandle, seats: usize) {
    for n in 0..seats {
        handle.send(seat(n), ClientMessage::Ready).await.unwrap();
    }
    assert!(handle.status().await.unwrap().started);
}

fn in_secs(secs: u64) -> Instant {
    Instant::now() + Duration::from_secs(secs)
}

#[tokio::test]
async fn a_token_opens_exactly_its_own_seat_of_its_own_game() {
    let registry = Registry::new(&ServerConfig::default());
    let first = registry.create(&pair()).unwrap();
    let second = registry
        .create(&requests(&["mono-red", "mono-white", "mono-green"]))
        .unwrap();
    assert_eq!((first.seats.len(), second.seats.len()), (2, 3));

    let mut seen = std::collections::HashSet::new();
    for created in [&first, &second] {
        for (seat, token) in &created.seats {
            assert!(
                seen.insert(token.expose().to_owned()),
                "a token was handed out twice"
            );
            let access = registry.authenticate(created.id, token.expose()).unwrap();
            assert_eq!((access.game, access.seat), (created.id, *seat));
            let other = if created.id == first.id {
                second.id
            } else {
                first.id
            };
            assert!(registry.authenticate(other, token.expose()).is_none());
        }
    }
    let (_, token) = &first.seats[0];
    for bad in [
        "",
        "x",
        &token.expose().to_uppercase(),
        &token.expose()[1..],
        &format!("{}0", token.expose()),
    ] {
        assert!(registry.authenticate(first.id, bad).is_none(), "{bad:?}");
    }
    // Neither the debug output of a creation nor a game's public facts contain a token.
    let shown = format!("{first:?}");
    for (_, token) in &first.seats {
        assert!(!shown.contains(token.expose()));
    }
    let public = registry.describe(first.id).await.unwrap();
    assert_eq!(
        public
            .seats
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Player 0", "Player 1"]
    );
    assert!(!format!("{public:?}").contains(token.expose()));
}

#[tokio::test]
async fn the_cap_counts_live_games_and_frees_a_slot_when_one_is_dropped() {
    let config = ServerConfig {
        max_games: 2,
        ..ServerConfig::default()
    };
    let registry = Registry::new(&config);
    let a = registry.create(&pair()).unwrap();
    let _b = registry.create(&pair()).unwrap();
    assert_eq!(registry.create(&pair()).unwrap_err(), CreateError::Full);
    assert_eq!(registry.len(), 2);

    // Two idle lobbies expire after an hour; then there is room again.
    assert_eq!(registry.sweep(in_secs(3_601)).await, 2);
    assert!(registry.is_empty());
    assert!(registry.create(&pair()).is_ok());
    assert!(registry.handle(a.id).is_none());
}

#[tokio::test]
async fn a_game_that_cannot_be_set_up_is_refused_not_registered() {
    let registry = Registry::new(&ServerConfig::default());
    let error = registry.create(&requests(&["mono-green"])).unwrap_err();
    assert!(matches!(
        error,
        CreateError::Start(StartError::Setup(SetupError::TooFewPlayers(1)))
    ));
    assert!(registry.is_empty());
}

#[tokio::test]
async fn shuffles_are_server_random_unless_a_test_fixes_the_seed() {
    async fn opening_hash(registry: &Registry) -> u64 {
        let created = registry.create(&pair()).unwrap();
        let handle = registry.handle(created.id).unwrap();
        start(&handle, 2).await;
        handle.status().await.unwrap().state_hash.unwrap()
    }
    let random = Registry::new(&ServerConfig::default());
    let hashes = [
        opening_hash(&random).await,
        opening_hash(&random).await,
        opening_hash(&random).await,
    ];
    assert!(
        hashes[0] != hashes[1] && hashes[1] != hashes[2] && hashes[0] != hashes[2],
        "{hashes:?}"
    );

    let fixed = registry_with_seed(9);
    assert_eq!(opening_hash(&fixed).await, opening_hash(&fixed).await);
    assert_ne!(
        opening_hash(&fixed).await,
        opening_hash(&registry_with_seed(10)).await
    );
}

fn registry_with_seed(seed: u64) -> Registry {
    Registry::new(&ServerConfig {
        fixed_seed: Some(seed),
        ..ServerConfig::default()
    })
}

#[tokio::test]
async fn finished_games_go_ten_minutes_after_a_sweep_first_sees_them_finished() {
    let registry = Registry::new(&ServerConfig::default());
    let created = registry.create(&pair()).unwrap();
    let handle = registry.handle(created.id).unwrap();
    start(&handle, 2).await;
    let (token, access) = {
        let (_, token) = &created.seats[1];
        (
            token.expose().to_owned(),
            registry.authenticate(created.id, token.expose()).unwrap(),
        )
    };
    let mut removed = access.removed.clone();

    // A running game is not touched, however long it takes.
    assert_eq!(registry.sweep(in_secs(1)).await, 0);
    handle.send(seat(0), ClientMessage::Concede).await.unwrap();
    assert!(handle.status().await.unwrap().result.is_some());

    let seen = Instant::now();
    assert_eq!(
        registry.sweep(seen).await,
        0,
        "the first sweep only notes the result"
    );
    assert_eq!(registry.sweep(seen + Duration::from_secs(599)).await, 0);
    assert!(
        registry
            .describe(created.id)
            .await
            .unwrap()
            .result
            .is_some()
    );
    assert!(removed.has_changed().is_ok(), "still registered");
    assert_eq!(registry.sweep(seen + Duration::from_secs(601)).await, 1);

    assert!(registry.describe(created.id).await.is_none());
    assert!(registry.authenticate(created.id, &token).is_none());
    // Sockets of a dropped game hear about it.
    assert!(removed.changed().await.is_err());
}

#[tokio::test]
async fn lobbies_that_never_start_go_after_an_hour() {
    let registry = Registry::new(&ServerConfig::default());
    let created = registry.create(&pair()).unwrap();
    let handle = registry.handle(created.id).unwrap();
    // Seats may be connected and one may be ready; the game still never starts.
    let _inbox = handle.join(seat(0)).await.unwrap();
    handle.send(seat(0), ClientMessage::Ready).await.unwrap();
    assert_eq!(registry.sweep(in_secs(3_599)).await, 0);
    assert_eq!(registry.sweep(in_secs(3_601)).await, 1);
    assert!(registry.is_empty());
}

#[tokio::test]
async fn started_games_stay_while_somebody_is_connected_and_go_a_day_after_everybody_left() {
    let config = ServerConfig {
        retention: Retention::default(),
        ..ServerConfig::default()
    };
    let registry = Registry::new(&config);
    let created = registry.create(&pair()).unwrap();
    let handle = registry.handle(created.id).unwrap();
    start(&handle, 2).await;
    let inbox = handle.join(seat(1)).await.unwrap();

    let t0 = Instant::now();
    let day = Duration::from_hours(24);
    let almost_a_day = Duration::from_mins(24 * 60 - 1);
    // While seat 1 is connected the game is never abandoned, however old.
    assert_eq!(registry.sweep(t0).await, 0);
    assert_eq!(registry.sweep(t0 + 3 * day).await, 0);
    // Once it leaves, the clock starts at the next sweep that notices.
    drop(inbox);
    let t1 = t0 + 3 * day;
    assert_eq!(registry.sweep(t1 + Duration::from_secs(60)).await, 0);
    assert_eq!(registry.sweep(t1 + almost_a_day).await, 0);
    assert_eq!(registry.sweep(t1 + day + Duration::from_secs(120)).await, 1);
    assert!(registry.is_empty());
}

#[tokio::test]
async fn the_janitor_task_sweeps_on_its_own_and_stops_on_request() {
    let retention = Retention {
        sweep_every: Duration::from_millis(20),
        lobby_after: Duration::from_millis(50),
        ..Retention::default()
    };
    let registry = Registry::new(&ServerConfig {
        retention,
        ..ServerConfig::default()
    });
    registry.create(&pair()).unwrap();
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let janitor = registry.spawn_janitor(stopped);
    for _ in 0..100 {
        if registry.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(registry.is_empty(), "the janitor never ran");
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), janitor)
        .await
        .unwrap()
        .unwrap();
}
