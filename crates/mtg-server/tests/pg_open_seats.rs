//! Goal: claims and releases are written to Postgres as they happen (the name, the SHA-256 of the token and the account of a claimed seat; nothing of a released one), racing claims get one winner each there too, a failed write changes nothing at the table, and a lobby with claims comes back after a restart with its names, its tokens, its accounts, its open seats, its visibility and its code, while a game that began with a seat nobody holds is retired.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Instant;

use common::{Client, roomy, seat, silent};
use futures_util::future::join_all;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::registry::{
    ClaimError, Created, Entrance, GameOptions, HOST, SeatClaim, SeatRequest, TableError,
};
use mtg_server::{
    GameId, GameRecords, RecordStatus, Registry, ResumeReport, SeatState, ServerConfig, UserId,
    Visibility, demo, migrate,
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

fn config() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(11),
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

async fn lobby(registry: &Registry, seats: usize, host: Option<UserId>) -> Created {
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(1),
    };
    registry
        .create_with(&requests(seats), options, host)
        .await
        .unwrap()
}

fn claim(name: &str, user: Option<UserId>) -> SeatClaim {
    SeatClaim {
        name: name.to_owned(),
        user,
    }
}

async fn reboot(pool: &PgPool) -> (Registry, ResumeReport) {
    let registry = Registry::with_database(&config(), pool.clone());
    let report = registry.resume().await.unwrap();
    (registry, report)
}

/// One row of `seats`: its state, name, token digest and account.
type Row = (String, String, Option<Vec<u8>>, Option<uuid::Uuid>);

async fn rows(pool: &PgPool, game: GameId) -> Vec<Row> {
    sqlx::query_as(
        "SELECT state, name, token_digest, user_id FROM seats WHERE game_id = $1 ORDER BY seat",
    )
    .bind(game.uuid())
    .fetch_all(pool)
    .await
    .unwrap()
}

fn digest(text: &str) -> Vec<u8> {
    Sha256::digest(text.as_bytes()).to_vec()
}

fn address() -> IpAddr {
    "192.0.2.1".parse().unwrap()
}

#[sqlx::test(migrations = false)]
async fn claims_releases_and_visibility_are_written_as_they_happen(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let (host, ann) = (
        insert_user(&pool, "host").await,
        insert_user(&pool, "ann").await,
    );
    let created = lobby(&registry, 4, Some(host)).await;
    let game = created.id;

    // Only the digest of each token is stored; the host's seat is the host's account's.
    let bo = registry
        .claim(game, Entrance::Listing, claim("Bo", None))
        .await
        .unwrap();
    let who = registry
        .claim(game, Entrance::Listing, claim("Ann", Some(ann)))
        .await
        .unwrap();
    let (bo_token, ann_token) = (bo.token.unwrap(), who.token.unwrap());
    let stored = rows(&pool, game).await;
    assert_eq!(stored[0].0, "issued");
    assert_eq!(stored[0].3, Some(host.uuid()));
    assert_eq!(
        stored[1],
        (
            "claimed".to_owned(),
            "Bo".to_owned(),
            Some(digest(bo_token.expose())),
            None
        )
    );
    assert_eq!(
        stored[2],
        (
            "claimed".to_owned(),
            "Ann".to_owned(),
            Some(digest(ann_token.expose())),
            Some(ann.uuid())
        )
    );
    assert_eq!(stored[3], ("open".to_owned(), String::new(), None, None));
    for token in [&bo_token, &ann_token] {
        for (_, name, _, _) in &stored {
            assert_ne!(name, token.expose());
        }
        let hits: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM seats t WHERE row_to_json(t)::text LIKE '%' || $1 || '%'",
        )
        .bind(token.expose())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hits, 0, "a token is stored in clear");
    }

    // The account sees the game on its list, with its seat.
    let listed = registry.games_of(ann).await.unwrap();
    assert_eq!((listed[0].id, listed[0].seat), (game, 2));

    // A release opens the row again: no name, token or account.
    registry.release(game, seat(2), seat(2)).await.unwrap();
    assert_eq!(
        rows(&pool, game).await[2],
        ("open".to_owned(), String::new(), None, None)
    );
    assert_eq!(registry.games_of(ann).await.unwrap(), []);

    // The visibility is stored too.
    registry
        .set_visibility(game, HOST, Visibility::Invite)
        .await
        .unwrap();
    let record = GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.visibility, Visibility::Invite);
    assert_eq!(record.seats[1].state, SeatState::Claimed);
    assert_eq!(record.seats[2].state, SeatState::Open);
}

#[sqlx::test(migrations = false)]
async fn racing_claims_get_one_winner_per_seat_in_the_database_too(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = lobby(&registry, 6, None).await;
    let game = created.id;

    let racers = (0..30).map(|n| {
        let registry = registry.clone();
        async move {
            let name = format!("Racer {n}");
            registry
                .claim(game, Entrance::Code, claim(&name, None))
                .await
        }
    });
    let answers = join_all(racers).await;
    let won: Vec<_> = answers.iter().filter_map(|a| a.as_ref().ok()).collect();
    assert_eq!(won.len(), 5);
    assert!(
        answers
            .iter()
            .filter_map(|a| a.as_ref().err())
            .all(|e| *e == ClaimError::Full)
    );
    let seats: HashSet<usize> = won.iter().map(|s| s.seat.index()).collect();
    assert_eq!(seats, (1..6).collect::<HashSet<_>>());

    let stored = rows(&pool, game).await;
    assert!(stored.iter().all(|row| row.0 != "open"));
    let digests: HashSet<_> = stored.iter().map(|row| row.2.clone()).collect();
    assert_eq!(digests.len(), 6, "every seat has a token of its own");
}

#[sqlx::test(migrations = false)]
async fn racing_claims_by_one_account_take_one_seat_in_the_database_too(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let ann = insert_user(&pool, "ann").await;
    let created = lobby(&registry, 4, None).await;
    let game = created.id;

    let racers = (0..10).map(|_| {
        let registry = registry.clone();
        async move {
            registry
                .claim(game, Entrance::Code, claim("Ann", Some(ann)))
                .await
        }
    });
    let answers: Vec<_> = join_all(racers)
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(answers.iter().filter(|s| s.token.is_some()).count(), 1);
    assert_eq!(
        answers
            .iter()
            .map(|s| s.seat.index())
            .collect::<HashSet<_>>()
            .len(),
        1
    );
    let owned = rows(&pool, game)
        .await
        .iter()
        .filter(|row| row.3 == Some(ann.uuid()))
        .count();
    assert_eq!(owned, 1);
}

#[sqlx::test(migrations = false)]
async fn a_lobby_with_claims_comes_back_after_a_restart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let ann = insert_user(&pool, "ann").await;
    let created = lobby(&registry, 4, None).await;
    let game = created.id;
    let code = created.code.expose().to_owned();
    let bo = registry
        .claim(game, Entrance::Listing, claim("Bo", None))
        .await
        .unwrap();
    let who = registry
        .claim(game, Entrance::Listing, claim("Ann", Some(ann)))
        .await
        .unwrap();
    let dropped = registry
        .claim(game, Entrance::Listing, claim("Gone", None))
        .await
        .unwrap();
    registry.release(game, dropped.seat, HOST).await.unwrap();
    // Everybody who is here presses Ready; none of it is stored.
    let handle = registry.handle(game).unwrap();
    handle.send(seat(0), ClientMessage::Ready).await.unwrap();
    assert_eq!(handle.status().await.unwrap().ready, [seat(0)]);

    let (again, report) = reboot(&pool).await;
    assert_eq!((report.resumed, report.lobbies), (1, 1));
    assert_eq!(again.visibility(game), Some(Visibility::Public));
    // Seat states and names come back as they were.
    let public = again.describe(game).await.unwrap();
    let seats: Vec<_> = public
        .seats
        .iter()
        .map(|s| (s.state, s.name.as_str()))
        .collect();
    assert_eq!(
        seats,
        [
            (SeatState::Issued, "Player 0"),
            (SeatState::Claimed, "Bo"),
            (SeatState::Claimed, "Ann"),
            (SeatState::Open, ""),
        ]
    );
    assert_eq!(public.ready, [], "readiness is not stored");
    // Old tokens still open their seats, the account still finds its own, and the released token is dead.
    let (bo_token, ann_token) = (bo.token.unwrap(), who.token.unwrap());
    assert_eq!(
        again.authenticate(game, bo_token.expose()).unwrap().seat,
        seat(1)
    );
    assert_eq!(
        again.authenticate(game, ann_token.expose()).unwrap().seat,
        seat(2)
    );
    assert!(
        again
            .authenticate(game, dropped.token.unwrap().expose())
            .is_none()
    );
    assert_eq!(again.authenticate_user(game, ann).unwrap().seat, seat(2));
    // The code finds the game, and the listing shows it with three of four seats filled.
    assert_eq!(
        again.find_by_code(&code, address(), Instant::now()),
        Ok(Some(game))
    );
    let listed = again.lobbies().await;
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].filled, listed[0].total), (3, 4));
    assert_eq!(listed[0].host, "Player 0");

    // The lobby tells a returning seat who sits where.
    let access = again.authenticate(game, bo_token.expose()).unwrap();
    let mut client = Client::join(&access.handle, 1).await;
    let ServerMessage::Lobby { seats, needed } = client
        .until(|m| matches!(m, ServerMessage::Lobby { .. }).then(|| m.clone()))
        .await
    else {
        unreachable!()
    };
    let at_table: Vec<_> = seats.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(at_table, ["Player 0", "Bo", "Ann", ""]);
    assert_eq!(needed, 4);

    // The open seat is still claimable after the restart, and the account still holds one seat.
    let cy = again
        .claim(game, Entrance::Code, claim("Cy", None))
        .await
        .unwrap();
    assert_eq!(cy.seat, seat(3));
    let own = again
        .claim(game, Entrance::Code, claim("Ann", Some(ann)))
        .await
        .unwrap();
    assert_eq!((own.seat, own.token), (seat(2), None));
}

#[sqlx::test(migrations = false)]
async fn a_lobby_that_started_keeps_its_claims_across_a_restart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = lobby(&registry, 2, None).await;
    let game = created.id;
    let bo = registry
        .claim(game, Entrance::Code, claim("Bo", None))
        .await
        .unwrap();
    let handle = registry.handle(game).unwrap();
    for number in 0..2 {
        handle
            .send(seat(number), ClientMessage::Ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);

    let (again, report) = reboot(&pool).await;
    assert_eq!((report.resumed, report.lobbies), (1, 0));
    assert!(again.describe(game).await.unwrap().started);
    let token = bo.token.unwrap();
    assert_eq!(
        again.authenticate(game, token.expose()).unwrap().seat,
        seat(1)
    );
    // Nothing is claimed or released any more.
    let late = again.claim(game, Entrance::Code, claim("Cy", None)).await;
    assert_eq!(late.unwrap_err(), ClaimError::Started);
    let release = again.release(game, seat(1), seat(1)).await;
    assert_eq!(release, Err(TableError::Started));
    assert_eq!(again.lobbies().await, []);
}

#[sqlx::test(migrations = false)]
async fn a_game_that_began_with_a_seat_nobody_holds_is_retired_at_boot(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = lobby(&registry, 2, None).await;
    let game = created.id;
    registry
        .claim(game, Entrance::Code, claim("Bo", None))
        .await
        .unwrap();
    let handle = registry.handle(game).unwrap();
    for number in 0..2 {
        handle
            .send(seat(number), ClientMessage::Ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);
    // Damage the record the way no code path does: the started game's second seat is open.
    sqlx::query(
        "UPDATE seats SET state = 'open', name = '', token_digest = NULL, user_id = NULL WHERE seat = 1",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (again, report) = reboot(&pool).await;
    assert_eq!(report.failed, [game]);
    assert!(again.handle(game).is_none());
    let record = GameRecords::new(pool).get(game).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Failed);
    assert!(
        record
            .failure
            .as_deref()
            .unwrap()
            .contains("seat nobody holds"),
        "{:?}",
        record.failure
    );
}

#[sqlx::test(migrations = false)]
async fn a_record_that_is_not_in_its_lobby_refuses_the_claim_and_changes_nothing(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = lobby(&registry, 2, None).await;
    let game = created.id;
    sqlx::query("UPDATE games SET status = 'running' WHERE id = $1")
        .bind(game.uuid())
        .execute(&pool)
        .await
        .unwrap();

    let answer = registry
        .claim(game, Entrance::Code, claim("Bo", None))
        .await;
    assert_eq!(answer.unwrap_err(), ClaimError::Full);
    let public = registry.describe(game).await.unwrap();
    assert_eq!(public.seats[1].state, SeatState::Open);
    assert_eq!(rows(&pool, game).await[1].0, "open");
    let handle = registry.handle(game).unwrap();
    assert_eq!(handle.status().await.unwrap().ready, []);
}

#[sqlx::test(migrations = false)]
async fn a_claim_the_database_cannot_save_leaves_the_seat_open(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = lobby(&registry, 2, None).await;
    let game = created.id;
    let handle = registry.handle(game).unwrap();
    let mut host = Client::join(&handle, 0).await;
    host.next().await;
    host.next().await;
    pool.close().await;

    let answer = registry
        .claim(game, Entrance::Code, claim("Bo", None))
        .await;
    assert!(matches!(answer, Err(ClaimError::Record(_))), "{answer:?}");
    let public = registry.describe(game).await.unwrap();
    assert_eq!(public.seats[1].state, SeatState::Open);
    assert_eq!(public.seats[1].name, "");
    // The table was not told of a name that did not stay.
    handle.status().await.unwrap();
    silent(&host.drain());
}

#[sqlx::test(migrations = false)]
async fn a_release_the_database_cannot_save_leaves_the_seat_claimed(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = lobby(&registry, 2, None).await;
    let game = created.id;
    let bo = registry
        .claim(game, Entrance::Code, claim("Bo", None))
        .await
        .unwrap();
    let token = bo.token.unwrap();
    let handle = registry.handle(game).unwrap();
    pool.close().await;

    let answer = registry.release(game, seat(1), seat(1)).await;
    assert!(matches!(answer, Err(TableError::Record(_))), "{answer:?}");
    let public = registry.describe(game).await.unwrap();
    assert_eq!(public.seats[1].state, SeatState::Claimed);
    assert_eq!(public.seats[1].name, "Bo");
    assert_eq!(
        registry.authenticate(game, token.expose()).unwrap().seat,
        seat(1)
    );
    // The table shows Bo's name again: a host who joins now sees it.
    let mut host = Client::join(&handle, 0).await;
    host.next().await;
    let ServerMessage::Lobby { seats, .. } = host.next().await else {
        panic!("the roster follows the welcome");
    };
    assert_eq!(seats[1].name, "Bo");
}

#[sqlx::test(migrations = false)]
async fn an_account_the_record_already_binds_is_handed_that_seat(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let ann = insert_user(&pool, "ann").await;
    let created = lobby(&registry, 3, None).await;
    let game = created.id;
    // Behind the registry's back the record gives Ann seat 2, so claiming seat 1 for her would
    // be her second seat, which the unique index refuses.
    sqlx::query(
        "UPDATE seats SET state = 'claimed', name = 'Ann', token_digest = $2, user_id = $3
         WHERE game_id = $1 AND seat = 2",
    )
    .bind(game.uuid())
    .bind(vec![9_u8; 32])
    .bind(ann.uuid())
    .execute(&pool)
    .await
    .unwrap();

    let answer = registry
        .claim(game, Entrance::Code, claim("Ann", Some(ann)))
        .await
        .unwrap();
    assert_eq!((answer.seat, answer.token), (seat(2), None));
    // Seat 1 stays open, in the record and at the table.
    assert_eq!(rows(&pool, game).await[1].0, "open");
    assert_eq!(
        registry.describe(game).await.unwrap().seats[1].state,
        SeatState::Open
    );
}

#[sqlx::test(migrations = false)]
async fn a_game_that_vanished_from_the_database_cannot_change_its_visibility(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = lobby(&registry, 2, None).await;
    sqlx::query("DELETE FROM games WHERE id = $1")
        .bind(created.id.uuid())
        .execute(&pool)
        .await
        .unwrap();

    let answer = registry
        .set_visibility(created.id, HOST, Visibility::Invite)
        .await;
    assert_eq!(answer, Err(TableError::NotFound));
    assert_eq!(registry.visibility(created.id), Some(Visibility::Public));
}

#[sqlx::test(migrations = false)]
async fn a_game_made_without_open_seats_never_has_a_claimable_seat(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&config(), pool.clone());
    let created = registry.create(&requests(2)).await.unwrap();
    let answer = registry
        .claim(created.id, Entrance::Code, claim("Bo", None))
        .await;
    assert_eq!(answer.unwrap_err(), ClaimError::Full);
    let stored = rows(&pool, created.id).await;
    assert!(stored.iter().all(|row| row.0 == "issued"));
}
