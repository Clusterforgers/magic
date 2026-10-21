//! Goal: on Postgres a lobby the host closed is stored as `expired`, so it is not loaded at the next boot, not listed among its players' games and not found by its code; the host closes it through the session as well as through a token, a joiner's account is refused, the sockets opened by a session are closed with 4002, and a close the database cannot save is a 503 that leaves the lobby as it was, still able to start.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;

use std::net::IpAddr;
use std::time::Instant;

use common::{roomy, seat};
use mtg_protocol::ClientMessage;
use mtg_server::registry::{Created, Entrance, GameOptions, HOST, SeatClaim, SeatRequest};
use mtg_server::{
    GameRecords, RecordStatus, Registry, ResumeReport, ServerConfig, UserId, Visibility, demo,
    migrate,
};
use net::lobby::{self, Made, bearer, call};
use net::{Browser, PASSWORD, TestServer, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;

fn config() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(13),
        game: roomy(),
        ..ServerConfig::default()
    }
}

fn requests(seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

/// An account made straight in the table.
async fn insert_user(pool: &PgPool, name: &str) -> UserId {
    let id = UserId::random();
    sqlx::query(
        "INSERT INTO users (id, username, username_key, password_hash)
         VALUES ($1, $2, lower($2), 'not a real hash')",
    )
    .bind(id.uuid())
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// A public lobby of `seats` seats of which only the host's, bound to `host`, is held.
async fn lobby(registry: &Registry, seats: usize, host: Option<UserId>) -> Created {
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(1),
        ..GameOptions::default()
    };
    registry
        .create_with(&requests(seats), options, host)
        .await
        .unwrap()
}

async fn reboot(pool: &PgPool) -> (Registry, ResumeReport) {
    let registry = Registry::with_database(&config(), pool.clone());
    let report = registry.resume().await.unwrap();
    (registry, report)
}

/// The stored status of `game`.
async fn status_of(pool: &PgPool, game: &str) -> String {
    let id: uuid::Uuid = game.parse().unwrap();
    sqlx::query_scalar("SELECT status FROM games WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn address() -> IpAddr {
    "192.0.2.1".parse().unwrap()
}

#[sqlx::test(migrations = false)]
async fn a_closed_lobby_is_stored_as_expired_and_does_not_come_back(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let host = insert_user(&pool, "host").await;
    let created = lobby(&registry, 3, Some(host)).await;
    let game = created.id;
    let bo = registry
        .claim(
            game,
            Entrance::Listing,
            SeatClaim {
                name: "Bo".to_owned(),
                user: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(status_of(&pool, &game.to_string()).await, "lobby");
    assert_eq!(registry.games_of(host).await.unwrap().len(), 1);

    registry.close(game, HOST).await.unwrap();

    // The record says the lobby is over; its seats stay as they were.
    assert_eq!(status_of(&pool, &game.to_string()).await, "expired");
    let record = GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, RecordStatus::Expired);
    assert_eq!(record.seats.len(), 3);
    assert_eq!(record.seats[1].name, "Bo");
    // An expired game is not among its players' games, and its code leads nowhere.
    assert_eq!(registry.games_of(host).await.unwrap().len(), 0);
    let found = registry.find_by_code(created.code.expose(), address(), Instant::now());
    assert_eq!(found.unwrap(), None);

    // After a restart nothing of it comes back: not the lobby, not the claimed seat's token.
    let (again, report) = reboot(&pool).await;
    assert_eq!(
        (
            report.resumed,
            report.lobbies,
            report.expired,
            report.failed
        ),
        (0, 0, 0, vec![])
    );
    assert!(again.handle(game).is_none());
    assert!(
        again
            .authenticate(game, bo.token.unwrap().expose())
            .is_none()
    );
    assert_eq!(again.lobbies().await, []);
    assert_eq!(status_of(&pool, &game.to_string()).await, "expired");
}

/// A migrated database behind a running server that does not count wrong codes or list requests.
async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    let mut config = bot_config();
    config.auth.limits.code_misses_per_ip = 100_000;
    config.auth.limits.lobby_requests_per_ip = 100_000;
    TestServer::start_on(pool.clone(), config).await
}

/// A game made by a signed-in creator: `seats` seats, the first held and bound to the creator.
async fn make_as(server: &TestServer, host: &Browser, seats: usize) -> Made {
    let body = lobby::body(&host.username, seats, 1, "public").to_string();
    let reply = host.post(server.addr, "/api/games", &body).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let json = reply.json();
    Made {
        id: json["game"].as_str().unwrap().to_owned(),
        code: json["code"].as_str().unwrap().to_owned(),
        tokens: json["seats"]
            .as_array()
            .unwrap()
            .iter()
            .map(|seat| seat["token"].as_str().unwrap().to_owned())
            .collect(),
    }
}

#[sqlx::test(migrations = false)]
async fn the_host_closes_through_the_session_and_a_joiners_account_is_refused(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let cy = register(server.addr, "Cyd", PASSWORD).await;
    let game = make_as(&server, &ann, 3).await;
    let path = format!("/api/games/{}", game.id);
    let joined = bo
        .send(
            server.addr,
            "POST",
            &format!("{path}/join"),
            &json!({}).to_string(),
            &[],
        )
        .await;
    assert_eq!(joined.status, 200, "{}", joined.text());

    // Both accounts have a socket open through their session alone.
    let ws = format!("/ws/{}", game.id);
    let mut ann_socket = net::open(server.addr, &ws, &[("Cookie", &ann.cookie)]).await;
    assert_eq!(ann_socket.welcome(&game.id).await.index(), 0);
    let mut bo_socket = net::open(server.addr, &ws, &[("Cookie", &bo.cookie)]).await;
    assert_eq!(bo_socket.welcome(&game.id).await.index(), 1);

    // The joiner's account is not the host; an account with no seat knows nothing of the game;
    // nobody signed in is asked to sign in.
    let refused = bo.delete(server.addr, &path).await;
    assert_eq!(
        (refused.status, refused.error_code().as_str()),
        (403, "forbidden")
    );
    let stranger = cy.delete(server.addr, &path).await;
    assert_eq!(
        (stranger.status, stranger.error_code().as_str()),
        (404, "not_found")
    );
    let anonymous = call(server.addr, "DELETE", &path, None, &[]).await;
    assert_eq!(
        (anonymous.status, anonymous.error_code().as_str()),
        (401, "unauthenticated")
    );
    assert_eq!(status_of(&pool, &game.id).await, "lobby");
    assert_eq!(server.registry.len(), 1);
    assert_eq!(ann.games(server.addr).await.len(), 1);

    let closed = ann.delete(server.addr, &path).await;
    assert!(
        closed.status == 204 && closed.body.is_empty(),
        "{} {}",
        closed.status,
        closed.text()
    );
    for socket in [&mut ann_socket, &mut bo_socket] {
        assert_eq!(socket.closed().await, Some(4002));
    }
    assert_eq!(status_of(&pool, &game.id).await, "expired");
    // Neither account lists it any more, and the session opens nothing of it.
    assert_eq!(ann.games(server.addr).await, Vec::<Value>::new());
    assert_eq!(bo.games(server.addr).await, Vec::<Value>::new());
    let (status, _) = net::refused_at(server.addr, &ws, &[("Cookie", &ann.cookie)]).await;
    assert_eq!(status, 404);
    let again = ann.delete(server.addr, &path).await;
    assert_eq!(again.status, 404);
}

#[sqlx::test(migrations = false)]
async fn a_close_the_database_cannot_save_is_a_503_and_leaves_the_lobby_as_it_was(pool: PgPool) {
    let server = start(&pool).await;
    // Two seats, both held, so that the table can start once it is ready.
    let game = lobby::make(server.addr, "Ann", 2, 2, "public").await;
    let path = format!("/api/games/{}", game.id);
    let host = bearer(game.host());
    let handle = server.registry.handle(game.id.parse().unwrap()).unwrap();

    // The write fails for as long as the table is away.
    sqlx::query("ALTER TABLE games RENAME TO games_away")
        .execute(&pool)
        .await
        .unwrap();
    let failed = call(
        server.addr,
        "DELETE",
        &path,
        None,
        &[("Authorization", &host)],
    )
    .await;
    assert_eq!(
        (failed.status, failed.error_code().as_str()),
        (503, "unavailable"),
        "{}",
        failed.text()
    );
    sqlx::query("ALTER TABLE games_away RENAME TO games")
        .execute(&pool)
        .await
        .unwrap();

    // Nothing changed: the game is there, listed, and its record still says lobby.
    let view = call(server.addr, "GET", &path, None, &[]).await;
    assert_eq!(view.status, 200, "{}", view.text());
    let listing = lobby::lobbies(server.addr, &[]).await;
    assert_eq!(lobby::listed(&listing), std::slice::from_ref(&game.id));
    assert_eq!(status_of(&pool, &game.id).await, "lobby");

    // The lobby is open again: the table can still start...
    for n in 0..2 {
        handle.send(seat(n), ClientMessage::Ready).await.unwrap();
    }
    assert!(handle.status().await.unwrap().started);
    // ...and a game that runs is not closed.
    let late = call(
        server.addr,
        "DELETE",
        &path,
        None,
        &[("Authorization", &host)],
    )
    .await;
    assert_eq!((late.status, late.error_code().as_str()), (409, "started"));
    let record: Value = call(server.addr, "GET", &path, None, &[]).await.json();
    assert_eq!(record["started"], true);
}
