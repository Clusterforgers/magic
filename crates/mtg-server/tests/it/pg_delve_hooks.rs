//! Goal: the test hook that lets a browser test end a Delves fight early is served only with `MTG_TEST_HOOKS`: without
//! it the route is not found; with it only the host reaches it, it waits (409) for a table that runs, and then the
//! boss concedes, so the game ends as a win of the party and the run reads it like any other: the loot is offered.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::pg_delve_forfeit::{call, fight_run, game_of, read};
use crate::{net, pg_commander};

use mtg_server::{GameRecords, RecordStatus, ServerConfig};
use net::lobby;
use net::{PASSWORD, TestServer, bot_config, register};
use serde_json::Value;
use sqlx::PgPool;

fn hook(id: &str) -> String {
    format!("/api/test/delves/{id}/boss-concedes")
}

async fn with_hooks(pool: &PgPool) -> TestServer {
    let config = ServerConfig {
        test_hooks: true,
        ..bot_config()
    };
    pg_commander::start_with(pool, config).await
}

/// Reads the run until `done` holds of it; the table settles on its own actor.
async fn until(
    server: &TestServer,
    id: &str,
    who: &net::Browser,
    done: impl Fn(&Value) -> bool,
) -> Value {
    for _ in 0..500 {
        let view = read(server, id, who).await;
        if done(&view) {
            return view;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the run never reached the state waited for");
}

#[sqlx::test(migrations = false)]
async fn without_test_hooks_the_route_is_not_found(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;
    lobby::start(&server.registry, &game, 3).await;
    let reply = call(&server, "POST", &hook(&id), &host, None).await;
    assert_eq!(reply.status, 404, "{}", reply.text());
    assert_eq!(game_of(&read(&server, &id, &host).await), game);
}

#[sqlx::test(migrations = false)]
async fn the_boss_concedes_a_running_fight_and_the_party_wins_it(pool: PgPool) {
    let server = with_hooks(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let guest = register(server.addr, "guest", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;

    let early = call(&server, "POST", &hook(&id), &host, None).await;
    assert_eq!(
        early.status,
        409,
        "the table has not started: {}",
        early.text()
    );
    lobby::start(&server.registry, &game, 3).await;
    let stranger = call(&server, "POST", &hook(&id), &guest, None).await;
    assert_eq!(stranger.status, 403, "{}", stranger.text());
    let missing = call(
        &server,
        "POST",
        &hook("00000000-0000-0000-0000-000000000000"),
        &host,
        None,
    )
    .await;
    assert_eq!(missing.status, 404);

    let conceded = call(&server, "POST", &hook(&id), &host, None).await;
    assert_eq!(conceded.status, 204, "{}", conceded.text());
    let view = until(&server, &id, &host, |view| view["run"]["status"] == "Room").await;
    assert_eq!(view["run"]["hearts"], 3, "a won fight costs nothing");
    assert_eq!(view["run"]["cleared"], serde_json::json!([0]));
    assert!(
        !view["run"]["heroes"][0]["offers"]
            .as_array()
            .unwrap()
            .is_empty(),
        "the loot waits for the hero"
    );
    let record = GameRecords::new(pool.clone())
        .get(game.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, RecordStatus::Finished);

    let later = call(&server, "POST", &hook(&id), &host, None).await;
    assert_eq!(later.status, 409, "no fight runs at camp: {}", later.text());
}
