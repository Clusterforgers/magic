//! Goal: a Delves fight whose game ends without a result is settled by the run and never hangs it: a fight lobby that
//! expires, a started game the janitor closes (a bot ally is no person) and a lobby the host closes each count as a
//! lost fight (a heart, every hero whole again before the first boss, a new game; defeat with no hearts left); the host's Retreat ends the
//! table and counts the same way, for the host alone and only in a fight.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use std::time::{Duration, Instant};

use mtg_server::{GameRecords, RecordStatus};
use net::lobby;
use net::{Browser, PASSWORD, Reply, TestServer, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

pub(crate) async fn call(
    server: &TestServer,
    method: &str,
    path: &str,
    who: &Browser,
    body: Option<Value>,
) -> Reply {
    let mut headers: Vec<(&str, &str)> = vec![("Cookie", who.cookie.as_str())];
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    request(server.addr, method, path, &headers, body.as_bytes()).await
}

pub(crate) async fn act(server: &TestServer, id: &str, who: &Browser, action: Value) -> Reply {
    call(
        server,
        "POST",
        &format!("/api/delves/{id}/act"),
        who,
        Some(action),
    )
    .await
}

pub(crate) async fn read(server: &TestServer, id: &str, who: &Browser) -> Value {
    call(server, "GET", &format!("/api/delves/{id}"), who, None)
        .await
        .json()
}

pub(crate) fn game_of(view: &Value) -> String {
    view["run"]["status"]["Fight"]["game"]
        .as_str()
        .unwrap_or_else(|| panic!("a fight with a game: {}", view["run"]["status"]))
        .to_owned()
}

fn last_line(view: &Value) -> &str {
    view["run"]["log"]
        .as_array()
        .and_then(|log| log.last())
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// A run in its first boss fight: the host with a bot ally, or with `guest` as the second hero. Its id and game.
pub(crate) async fn fight_run(
    server: &TestServer,
    host: &Browser,
    guest: Option<&Browser>,
) -> (String, String) {
    let created = call(
        server,
        "POST",
        "/api/delves",
        host,
        Some(json!({"heat": 0, "seed": "FORFEIT"})),
    )
    .await;
    assert_eq!(created.status, 201, "{}", created.text());
    let created = created.json();
    let id = created["id"].as_str().unwrap().to_owned();
    let code = created["code"].as_str().unwrap();
    let second = if let Some(guest) = guest {
        let path = format!("/api/delves/join/{code}");
        let joined = call(server, "POST", &path, guest, None).await;
        assert_eq!(joined.status, 200, "{}", joined.text());
        guest
    } else {
        let ally = json!({"kind": "add_bot", "level": "Normal"});
        let bot = act(server, &id, host, ally).await;
        assert_eq!(bot.status, 200, "{}", bot.text());
        host
    };
    for (who, hero) in [(host, 0), (second, 1)] {
        let chosen = act(
            server,
            &id,
            who,
            json!({"kind": "choose_deck", "hero": hero, "deck": pg_commander::on(pg_commander::BEAR)}),
        )
        .await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    let started = act(server, &id, host, json!({"kind": "start"})).await;
    assert_eq!(started.status, 200, "{}", started.text());
    let game = game_of(&started.json());
    (id, game)
}

async fn status_of(pool: &PgPool, game: &str) -> RecordStatus {
    GameRecords::new(pool.clone())
        .get(game.parse().unwrap())
        .await
        .unwrap()
        .unwrap()
        .status
}

/// What a fight against the first boss lost without a result leaves: one heart gone, every hero whole again, a new game
/// waiting, and the chronicle saying how.
fn assert_lost(view: &Value, old: &str, how: &str) {
    assert_eq!(view["run"]["hearts"], 2, "{}", view["run"]["log"]);
    assert_ne!(game_of(view), old, "the fight is made again");
    for hero in view["run"]["heroes"].as_array().unwrap() {
        assert_eq!(
            hero["life"], 40,
            "every hero regroups at full life before the first boss"
        );
    }
    assert!(last_line(view).starts_with(how), "{}", last_line(view));
}

#[sqlx::test(migrations = false)]
async fn a_fight_lobby_that_expires_counts_as_lost_and_is_made_again(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;
    assert_eq!(read(&server, &id, &host).await["run"]["hearts"], 3);

    // Nobody readied the table for two hours: the janitor drops the lobby.
    let dropped = server
        .registry
        .sweep(Instant::now() + Duration::from_hours(2))
        .await;
    assert_eq!(dropped, 1);
    assert_eq!(status_of(&pool, &game).await, RecordStatus::Expired);

    let view = read(&server, &id, &host).await;
    assert_lost(&view, &game, "The party lost its table");
    assert!(
        server
            .registry
            .handle(game_of(&view).parse().unwrap())
            .is_some(),
        "the new game is alive"
    );
    // A second read does not lose the fight again.
    let again = read(&server, &id, &host).await;
    assert_eq!(again["run"]["hearts"], 2);
    assert_eq!(game_of(&again), game_of(&view));
}

#[sqlx::test(migrations = false)]
async fn a_started_fight_the_janitor_closes_counts_as_lost_though_bot_allies_hold_seats(
    pool: PgPool,
) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;
    // The hero, the ally and the boss are three seats; the last Ready starts the game.
    lobby::start(&server.registry, &game, 3).await;
    assert_eq!(status_of(&pool, &game).await, RecordStatus::Running);
    // While it may yet end, the run waits.
    assert_eq!(game_of(&read(&server, &id, &host).await), game);

    let dropped = server
        .registry
        .sweep(Instant::now() + Duration::from_hours(2))
        .await;
    assert_eq!(dropped, 1, "no person sat at the table");
    assert_eq!(status_of(&pool, &game).await, RecordStatus::Expired);
    assert_lost(
        &read(&server, &id, &host).await,
        &game,
        "The party lost its table",
    );
}

#[sqlx::test(migrations = false)]
async fn a_fight_lobby_the_host_closes_counts_as_lost(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;
    let closed = call(
        &server,
        "DELETE",
        &format!("/api/games/{game}"),
        &host,
        None,
    )
    .await;
    assert_eq!(closed.status, 204, "{}", closed.text());
    assert_eq!(status_of(&pool, &game).await, RecordStatus::Expired);
    assert_lost(
        &read(&server, &id, &host).await,
        &game,
        "The party lost its table",
    );
}

#[sqlx::test(migrations = false)]
async fn the_host_retreats_and_the_table_ends_with_the_fight_lost(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;
    lobby::start(&server.registry, &game, 3).await;

    let retreat = act(&server, &id, &host, json!({"kind": "retreat"})).await;
    assert_eq!(retreat.status, 200, "{}", retreat.text());
    let view = retreat.json();
    assert_lost(&view, &game, "The party retreated");
    assert_eq!(status_of(&pool, &game).await, RecordStatus::Aborted);
    assert!(
        server.registry.handle(game.parse().unwrap()).is_none(),
        "nobody plays on in the table the run gave up"
    );
    // The retreat is part of the run: a later read finds the same state.
    let later = read(&server, &id, &host).await;
    assert_eq!(game_of(&later), game_of(&view));
    assert_eq!(later["run"]["hearts"], 2);
}

#[sqlx::test(migrations = false)]
async fn only_the_host_retreats_and_only_from_a_fight(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let guest = register(server.addr, "guest", PASSWORD).await;
    let stranger = register(server.addr, "stranger", PASSWORD).await;

    let lobby_run = call(
        &server,
        "POST",
        "/api/delves",
        &host,
        Some(json!({"heat": 0, "seed": "LOBBY"})),
    )
    .await
    .json();
    let lobby_id = lobby_run["id"].as_str().unwrap();
    let none = act(&server, lobby_id, &host, json!({"kind": "retreat"})).await;
    assert_eq!(none.status, 409, "the lobby is no fight: {}", none.text());

    let (id, game) = fight_run(&server, &host, Some(&guest)).await;
    for (who, status) in [(&guest, 403), (&stranger, 403)] {
        let refused = act(&server, &id, who, json!({"kind": "retreat"})).await;
        assert_eq!(refused.status, status, "{}", refused.text());
    }
    let view = read(&server, &id, &host).await;
    assert_eq!(game_of(&view), game, "nothing changed");
    assert_eq!(view["run"]["hearts"], 3);
    assert!(server.registry.handle(game.parse().unwrap()).is_some());
}

#[sqlx::test(migrations = false)]
async fn retreating_with_the_last_heart_ends_the_run_in_defeat(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, mut game) = fight_run(&server, &host, None).await;
    for left in [2, 1] {
        let view = act(&server, &id, &host, json!({"kind": "retreat"}))
            .await
            .json();
        assert_eq!(view["run"]["hearts"], left);
        let next = game_of(&view);
        assert_ne!(next, game);
        game = next;
    }
    let last = act(&server, &id, &host, json!({"kind": "retreat"})).await;
    assert_eq!(last.status, 200, "{}", last.text());
    let view = last.json();
    assert_eq!(view["run"]["status"], "Defeat");
    assert_eq!(view["run"]["hearts"], 0);
    assert_eq!(status_of(&pool, &game).await, RecordStatus::Aborted);
    let more = act(&server, &id, &host, json!({"kind": "retreat"})).await;
    assert_eq!(more.status, 409, "a lost run has no fight to leave");
}
