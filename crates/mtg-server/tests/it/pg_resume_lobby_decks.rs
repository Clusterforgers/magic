//! Goal: a lobby whose seats hold decks of oracle ids (precons, a stored Commander deck, a Delves fight's heroes and boss)
//! comes back after a restart with every seat on its deck and the name the table was told: Ready starts it under the
//! recipe it was made with, which the start leaves as it is, and the started game comes back after another restart.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::pg_delve_forfeit::fight_run;
use crate::{net, pg_commander};

use mtg_server::{GameRecords, RecordStatus};
use net::lobby;
use net::{PASSWORD, TestServer, bot_config, register};
use pg_commander::{BEAR, BEAST, create_body, led_by, on, source, store};
use serde_json::{Value, json};
use sqlx::PgPool;

/// How the lobby was made.
#[derive(Debug, Clone, Copy)]
enum Made {
    /// Two seats on precons.
    Precons,
    /// The creator's stored deck and a precon.
    Stored,
    /// A Delves boss fight, as the run makes it.
    Delve,
}

async fn record(pool: &PgPool, game: &str) -> (Value, RecordStatus) {
    let record = GameRecords::new(pool.clone())
        .get(game.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    (record.recipe, record.status)
}

/// The lobby `made` makes on `server`: its game id.
async fn lobby_of(server: &TestServer, made: Made) -> String {
    let ann = register(server.addr, "ann", PASSWORD).await;
    let seats = match made {
        Made::Delve => return fight_run(server, &ann, None).await.1,
        Made::Precons => [on(BEAST), on(BEAR)],
        Made::Stored => [
            store(server, &ann, &led_by("Questing Beast")).await,
            on(BEAR),
        ],
    };
    let seats: Vec<Value> = seats
        .iter()
        .enumerate()
        .map(|(seat, deck)| json!({"name": format!("Player {seat}"), "deck": deck}))
        .collect();
    ann.create_game(server.addr, &create_body(&seats, seats.len()))
        .await
        .0
}

/// Stops `server` and starts another on the same database, which resumes what it left.
async fn restart(mut server: TestServer, pool: &PgPool) -> TestServer {
    server.shutdown();
    server.finished().await.unwrap();
    drop(server);
    TestServer::start_with_cards(pool.clone(), bot_config(), source(pool)).await
}

#[sqlx::test(migrations = false)]
async fn a_lobby_of_precons_starts_on_them_after_a_restart(pool: PgPool) {
    starts_on_its_decks_after_a_restart(pool, Made::Precons).await;
}

#[sqlx::test(migrations = false)]
async fn a_lobby_of_a_stored_deck_starts_on_it_after_a_restart(pool: PgPool) {
    starts_on_its_decks_after_a_restart(pool, Made::Stored).await;
}

#[sqlx::test(migrations = false)]
async fn a_delves_fight_lobby_starts_on_the_heroes_and_boss_decks_after_a_restart(pool: PgPool) {
    starts_on_its_decks_after_a_restart(pool, Made::Delve).await;
}

async fn starts_on_its_decks_after_a_restart(pool: PgPool, made: Made) {
    let server = pg_commander::start(&pool).await;
    let game = lobby_of(&server, made).await;
    let (before, status) = record(&pool, &game).await;
    assert_eq!(status, RecordStatus::Lobby, "{made:?}");
    let decks = before["decks"].as_array().unwrap();
    for deck in decks {
        assert_eq!(deck["kind"], "oracle", "{made:?}: {deck}");
        assert!(deck["note"]["name"].is_string(), "{made:?}: {deck}");
    }

    let server = restart(server, &pool).await;
    lobby::start(&server.registry, &game, decks.len()).await;
    let (after, status) = record(&pool, &game).await;
    assert_eq!(status, RecordStatus::Running, "{made:?}");
    assert_eq!(
        after, before,
        "{made:?}: the game starts under the recipe it was made with"
    );

    let server = restart(server, &pool).await;
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    assert!(handle.status().await.unwrap().started, "{made:?}");
    assert_eq!(
        record(&pool, &game).await.1,
        RecordStatus::Running,
        "{made:?}"
    );
}
