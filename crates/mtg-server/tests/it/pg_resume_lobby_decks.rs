//! Goal: a lobby whose seats hold decks of oracle ids (precons, a stored Commander deck, a Delves fight's heroes and boss)
//! comes back after a restart with every seat on its deck and the name the table was told: Ready starts it under the
//! recipe it was made with, which the start leaves as it is, and the started game comes back after another restart. A deck a seat chooses in the lobby is stored at once and the lobby comes back on it.
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

/// The commanders the public listing names for `game`, by seat.
async fn listed_leaders(server: &TestServer, game: &str) -> Vec<(u64, String)> {
    let listing = lobby::lobbies(server.addr, &[]).await.json();
    let row = listing["games"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["game"] == game)
        .unwrap()
        .clone();
    let mut leaders: Vec<(u64, String)> = row["commanders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["seat"].as_u64().unwrap(),
                c["name"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    leaders.sort();
    leaders
}

#[sqlx::test(migrations = false)]
async fn a_deck_chosen_in_the_lobby_is_stored_at_once_and_kept_by_a_restart(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    // Seat 0 is made on a precon and seat 1 without a deck, which the recipe names by its stand-in.
    let seats = [
        json!({"name": "Ann", "deck": on(BEAR)}),
        json!({"name": "Bo"}),
    ];
    let (game, tokens) = ann.create_game(server.addr, &create_body(&seats, 2)).await;
    let (made, _) = record(&pool, &game).await;
    for (seat, deck) in [(0, BEAST), (1, BEAR)] {
        let chosen = lobby::call(
            server.addr,
            "PUT",
            &format!("/api/games/{game}/seats/{seat}/deck"),
            Some(&json!({ "deck": on(deck) }).to_string()),
            &[("Authorization", &lobby::bearer(&tokens[seat]))],
        )
        .await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    let (stored, status) = record(&pool, &game).await;
    assert_eq!(status, RecordStatus::Lobby);
    assert_ne!(
        stored["decks"], made["decks"],
        "the choices are stored at once"
    );
    assert_eq!(stored["decks"][0]["note"]["name"], "Beast Pack");
    assert_eq!(stored["decks"][1]["note"]["name"], "Bear Pack");
    let chosen = vec![
        (0, "Questing Beast".to_owned()),
        (1, "Grizzly Bears".to_owned()),
    ];
    assert_eq!(listed_leaders(&server, &game).await, chosen);

    let server = restart(server, &pool).await;
    assert_eq!(
        listed_leaders(&server, &game).await,
        chosen,
        "the lobby comes back on the decks chosen before the restart"
    );
    lobby::start(&server.registry, &game, 2).await;
    let (after, status) = record(&pool, &game).await;
    assert_eq!(status, RecordStatus::Running);
    assert_eq!(after, stored, "the game starts under the stored choices");
}
