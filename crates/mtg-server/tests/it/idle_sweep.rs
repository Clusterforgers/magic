//! Goal: the janitor closes a started game that no person sits at and in which no answer has been taken for the idle time, recording it `expired` so it never resumes and its lists show it ended; a game with a person connected, a recent answer or a result is left alone; a bot holding its seat is not a person, and a game with bots closes a quarter hour after its last person left however its bots play on.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{self, Client};

use std::time::{Duration, Instant};

use common::roomy;
use mtg_core::ids::PlayerId;
use mtg_protocol::ClientMessage;
use mtg_server::bots::{BotLevel, BotSeat};
use mtg_server::config::Retention;
use mtg_server::registry::{GameOptions, SeatRequest};
use mtg_server::{GameId, GameRecords, RecordStatus, Registry, ServerConfig, demo, migrate};
use rstest::rstest;
use sqlx::PgPool;

const HOUR: Duration = Duration::from_hours(1);

fn requests() -> Vec<SeatRequest> {
    ["mono-green", "mono-red"]
        .iter()
        .enumerate()
        .map(|(i, id)| SeatRequest {
            name: format!("Player {i}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

fn config() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(11),
        game: roomy(),
        ..ServerConfig::default()
    }
}

/// A started two-seat game with both seats connected, and the clients holding them.
async fn running(registry: &Registry) -> (GameId, Vec<Client>) {
    let created = registry.create(&requests()).await.unwrap();
    let handle = registry.handle(created.id).unwrap();
    let (clients, _) = common::start_table(&handle, 2).await;
    (created.id, clients)
}

/// Seat `first` answers its pending decision, which grows the game's log.
async fn answer(registry: &Registry, game: GameId, clients: &mut [Client], first: usize) {
    let handle = registry.handle(game).unwrap();
    let (id, decision) = clients[first].decision().await;
    let answer = common::passive(&decision);
    let message = ClientMessage::Answer {
        decision: id,
        answer,
    };
    clients[first].say(&handle, message).await;
    handle.status().await.unwrap();
}

async fn status(pool: &PgPool, game: GameId) -> RecordStatus {
    GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .unwrap()
        .status
}

/// Pauses the sweep clock for `minutes` past `base`.
fn later(base: Instant, minutes: u64) -> Instant {
    base + Duration::from_mins(minutes)
}

#[sqlx::test(migrations = false)]
async fn a_game_nobody_sits_at_and_nobody_answers_in_is_closed_after_an_hour(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let (game, clients) = running(&registry).await;
    let base = Instant::now();
    // Somebody is there at the first sweep.
    assert_eq!(registry.sweep(base).await, 0);
    drop(clients);

    // The hour counts from the last time a person was seen.
    assert_eq!(registry.sweep(later(base, 5)).await, 0);
    assert_eq!(registry.sweep(later(base, 59)).await, 0);
    assert_eq!(status(&pool, game).await, RecordStatus::Running);
    assert_eq!(registry.sweep(later(base, 61)).await, 1);

    assert!(registry.handle(game).is_none());
    assert_eq!(status(&pool, game).await, RecordStatus::Expired);
    // Closed for good: a restart does not bring it back.
    let again = Registry::with_database(&config(), pool.clone());
    let report = again.resume().await.unwrap();
    assert_eq!((report.resumed, report.expired), (0, 0));
}

#[sqlx::test(migrations = false)]
async fn a_game_with_a_person_connected_stays_however_long_it_is_quiet(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let (game, _clients) = running(&registry).await;
    let base = Instant::now();
    for hours in [0, 2, 10, 30, 100] {
        assert_eq!(registry.sweep(base + hours * HOUR).await, 0, "{hours} h");
    }
    assert!(registry.handle(game).is_some());
    assert_eq!(status(&pool, game).await, RecordStatus::Running);
}

#[sqlx::test(migrations = false)]
async fn a_recent_answer_keeps_a_game_nobody_is_connected_to_for_another_hour(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let (game, mut clients) = running(&registry).await;
    // A second, silent game shows what the answer changes.
    let (silent, silent_clients) = running(&registry).await;
    let base = Instant::now();
    assert_eq!(registry.sweep(base).await, 0);

    let first = registry
        .handle(game)
        .unwrap()
        .status()
        .await
        .unwrap()
        .waiting_on
        .unwrap()
        .index();
    answer(&registry, game, &mut clients, first).await;
    drop((clients, silent_clients));

    // The sweep finds the answer at 50 minutes; both tables are empty by then.
    assert_eq!(registry.sweep(later(base, 50)).await, 0);
    // The silent game's hour since its people left is up; the answered game's has 50 minutes to go.
    assert_eq!(registry.sweep(later(base, 61)).await, 1);
    assert_eq!(status(&pool, silent).await, RecordStatus::Expired);
    assert_eq!(status(&pool, game).await, RecordStatus::Running);
    assert_eq!(registry.sweep(later(base, 109)).await, 0);
    assert_eq!(registry.sweep(later(base, 111)).await, 1);
    assert_eq!(status(&pool, game).await, RecordStatus::Expired);
}

#[sqlx::test(migrations = false)]
async fn a_finished_game_keeps_its_result_and_is_dropped_by_its_own_rule(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = registry.create(&requests()).await.unwrap();
    let handle = registry.handle(created.id).unwrap();
    common::play_table(&handle, 2, 11).await;
    assert_eq!(status(&pool, created.id).await, RecordStatus::Finished);

    // Nobody is connected and a day has gone, yet a finished game is not an abandoned one.
    let base = Instant::now() + 24 * HOUR;
    assert_eq!(registry.sweep(base).await, 0);
    assert!(registry.handle(created.id).is_some());
    assert_eq!(registry.sweep(later(base, 11)).await, 1);

    let record = GameRecords::new(pool.clone())
        .get(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, RecordStatus::Finished);
    assert!(record.result.is_some(), "the result stays");
}

#[sqlx::test(migrations = false)]
async fn the_janitor_task_closes_an_idle_running_game_on_its_own(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let retention = Retention {
        sweep_every: Duration::from_millis(20),
        abandoned_after: Duration::from_millis(150),
        ..Retention::default()
    };
    let registry = Registry::with_database(
        &ServerConfig {
            retention,
            ..config()
        },
        pool.clone(),
    );
    let (game, clients) = running(&registry).await;
    drop(clients);
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let janitor = registry.spawn_janitor(stopped);
    for _ in 0..200 {
        if registry.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(registry.is_empty(), "the janitor never closed the game");
    assert_eq!(status(&pool, game).await, RecordStatus::Expired);
    stop.send(true).unwrap();
    janitor.await.unwrap();
}

/// Waits until the game runs, every bot of `seats` holds its seat and the log has been still for a while.
async fn settled(handle: &mtg_server::GameHandle, seats: usize) {
    let mut quiet = 0;
    let mut events = 0;
    while quiet < 5 {
        tokio::time::sleep(Duration::from_millis(40)).await;
        let status = handle.status().await.unwrap();
        let bots_here =
            (1..seats).all(|seat| status.connected.contains(&PlayerId::from_index(seat)));
        quiet = if status.started && bots_here && status.events == events {
            quiet + 1
        } else {
            0
        };
        events = status.events;
    }
}

#[rstest]
#[case::one_bot(2)]
#[case::three_bots(4)]
#[tokio::test]
async fn bots_left_alone_are_closed_a_quarter_hour_after_the_last_person_left(
    #[case] seats: usize,
) {
    let registry = Registry::new(&ServerConfig {
        game: roomy(),
        bot_delay: Duration::from_millis(5),
        ..ServerConfig::default()
    });
    let decks = ["mono-green", "mono-red"];
    let requests: Vec<SeatRequest> = (0..seats)
        .map(|seat| SeatRequest {
            name: format!("Player {seat}"),
            deck: demo::find(decks[seat % 2]).unwrap(),
        })
        .collect();
    let options = GameOptions {
        bots: (1..seats)
            .map(|seat| BotSeat {
                seat: PlayerId::from_index(seat),
                level: BotLevel::Easy,
            })
            .collect(),
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests, options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut human = Client::join(&handle, 0).await;
    human.say(&handle, ClientMessage::Ready).await;
    settled(&handle, seats).await;

    // With a person at the table it stays, however long the game runs.
    let base = Instant::now();
    assert_eq!(registry.sweep(base).await, 0);
    let left = base + 10 * HOUR;
    assert_eq!(registry.sweep(left).await, 0);

    // The person answers and leaves; the bots play on, and the next sweep sees the log grown.
    let (id, decision) = human.decision().await;
    let answer = common::passive(&decision);
    let message = ClientMessage::Answer {
        decision: id,
        answer,
    };
    human.say(&handle, message).await;
    settled(&handle, seats).await;
    drop(human);
    assert_eq!(registry.sweep(later(left, 14)).await, 0);
    // Moves at the table are no person: a quarter hour after the last one left, the table closes.
    assert_eq!(registry.sweep(later(left, 16)).await, 1);
    assert!(registry.is_empty());
}
