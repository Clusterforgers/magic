//! Goal: visibility and seat states are stored with the game: a game that existed before the migration is invite-only with its seats issued and still resumes, a new game keeps the visibility and the open seats it was made with across a restart, and the database refuses a visibility, an open seat with a token or a claimed seat without one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Client, roomy, seat};
use mtg_protocol::ServerMessage;
use mtg_server::registry::{GameOptions, SeatRequest};
use mtg_server::{
    GameRecords, RecordStatus, Registry, ResumeReport, SeatState, ServerConfig, Visibility, demo,
    migrate,
};
use sqlx::PgPool;
use uuid::Uuid;

fn config() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(5),
        game: roomy(),
        ..ServerConfig::default()
    }
}

fn requests(names: &[&str]) -> Vec<SeatRequest> {
    names
        .iter()
        .enumerate()
        .map(|(n, name)| SeatRequest {
            name: (*name).to_owned(),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

/// A new process on the same database.
async fn reboot(pool: &PgPool) -> (Registry, ResumeReport) {
    let registry = Registry::with_database(&config(), pool.clone());
    let report = registry.resume().await.unwrap();
    (registry, report)
}

#[sqlx::test(migrations = false)]
async fn a_game_stored_before_visibility_existed_is_invite_only_and_resumes(pool: PgPool) {
    migrate(&pool).await.unwrap();
    // Rows as the server wrote them before the migration: no visibility, no code, no seat state.
    let game = Uuid::new_v4();
    let recipe = serde_json::json!({
        "format": "normal",
        "seed": 3,
        "decks": [{"kind": "demo", "id": "mono-green"}, {"kind": "demo", "id": "mono-red"}],
    });
    sqlx::query("INSERT INTO games (id, status, recipe) VALUES ($1, 'lobby', $2)")
        .bind(game)
        .bind(recipe)
        .execute(&pool)
        .await
        .unwrap();
    for (number, name) in [(0_i32, "Ann"), (1, "Bo")] {
        sqlx::query(
            "INSERT INTO seats (game_id, seat, name, token_digest) VALUES ($1, $2, $3, $4)",
        )
        .bind(game)
        .bind(number)
        .bind(name)
        .bind(vec![u8::try_from(number).unwrap(); 32])
        .execute(&pool)
        .await
        .unwrap();
    }

    let record = GameRecords::new(pool.clone())
        .get(game.into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.visibility, Visibility::Invite);
    assert_eq!(record.code_digest, None);
    assert!(
        record
            .seats
            .iter()
            .all(|seat| seat.state == SeatState::Issued && seat.token_digest.is_some())
    );

    let (registry, report) = reboot(&pool).await;
    assert_eq!((report.resumed, report.lobbies), (1, 1));
    assert_eq!(registry.visibility(game.into()), Some(Visibility::Invite));
    let public = registry.describe(game.into()).await.unwrap();
    assert!(public.seats.iter().all(|seat| !seat.open));
}

#[sqlx::test(migrations = false)]
async fn visibility_and_open_seats_are_stored_and_survive_a_restart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let default = registry.create(&requests(&["Ann", "Bo"])).await.unwrap();
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(1),
    };
    let listed = registry
        .create_with(&requests(&["Cy", "ignored", "ignored too"]), options, None)
        .await
        .unwrap();

    let records = GameRecords::new(pool.clone());
    let stored = records.get(default.id).await.unwrap().unwrap();
    assert_eq!(stored.visibility, Visibility::Invite);
    let stored = records.get(listed.id).await.unwrap().unwrap();
    assert_eq!(stored.visibility, Visibility::Public);
    assert_eq!(stored.status, RecordStatus::Lobby);
    let states: Vec<_> = stored.seats.iter().map(|seat| seat.state).collect();
    assert_eq!(
        states,
        [SeatState::Issued, SeatState::Open, SeatState::Open]
    );
    let names: Vec<_> = stored.seats.iter().map(|seat| seat.name.as_str()).collect();
    assert_eq!(names, ["Cy", "", ""]);
    let tokens: Vec<_> = stored
        .seats
        .iter()
        .map(|seat| seat.token_digest.is_some())
        .collect();
    assert_eq!(tokens, [true, false, false]);

    // A new process finds the same visibility, the same open seats and the same names.
    let (again, report) = reboot(&pool).await;
    assert_eq!((report.resumed, report.lobbies), (2, 2));
    assert_eq!(again.visibility(default.id), Some(Visibility::Invite));
    assert_eq!(again.visibility(listed.id), Some(Visibility::Public));
    let public = again.describe(listed.id).await.unwrap();
    assert_eq!(public.visibility, Visibility::Public);
    let open: Vec<_> = public.seats.iter().map(|seat| seat.open).collect();
    assert_eq!(open, [false, true, true]);

    // The held seat's old token opens it, and the lobby lists the open seats as nameless.
    let (_, token) = &listed.seats[0];
    let access = again.authenticate(listed.id, token.expose()).unwrap();
    assert_eq!(access.seat, seat(0));
    let mut host = Client::join(&access.handle, 0).await;
    let ServerMessage::Lobby { seats, needed } = host
        .until(|m| matches!(m, ServerMessage::Lobby { .. }).then(|| m.clone()))
        .await
    else {
        unreachable!()
    };
    let names: Vec<_> = seats.iter().map(|seat| seat.name.as_str()).collect();
    assert_eq!(names, ["Cy", "", ""]);
    assert_eq!(needed, 3);
}

/// Statements the server never issues and what each would store, which the schema must refuse.
const REFUSED: [(&str, &str); 6] = [
    (
        "UPDATE games SET visibility = 'private'",
        "a visibility the server does not know",
    ),
    (
        "UPDATE seats SET state = 'open' WHERE seat = 1",
        "an open seat that still has a token",
    ),
    (
        "UPDATE seats SET state = 'claimed', token_digest = NULL WHERE seat = 1",
        "a claimed seat with no token",
    ),
    (
        "UPDATE seats SET token_digest = NULL WHERE seat = 1",
        "an issued seat with no token",
    ),
    (
        "UPDATE seats SET state = 'reserved' WHERE seat = 1",
        "a seat state the server does not know",
    ),
    (
        "UPDATE games SET code_digest = '\\x0102'",
        "a join code digest of the wrong size",
    ),
];

#[sqlx::test(migrations = false)]
async fn the_database_refuses_what_the_server_never_writes(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    registry.create(&requests(&["Ann", "Bo"])).await.unwrap();
    for (statement, what) in REFUSED {
        let refused = sqlx::query(sqlx::AssertSqlSafe(statement.to_owned()))
            .execute(&pool)
            .await;
        assert!(refused.is_err(), "the database stored {what}");
    }
}

#[sqlx::test(migrations = false)]
async fn an_open_seat_is_stored_without_a_token_or_an_account(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let options = GameOptions {
        visibility: Visibility::Invite,
        held: Some(1),
    };
    let created = registry
        .create_with(&requests(&["Ann", "Bo"]), options, None)
        .await
        .unwrap();
    let (state, digest, user): (String, Option<Vec<u8>>, Option<Uuid>) = sqlx::query_as(
        "SELECT state, token_digest, user_id FROM seats WHERE game_id = $1 AND seat = 1",
    )
    .bind(created.id.uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((state.as_str(), digest, user), ("open", None, None));
}
