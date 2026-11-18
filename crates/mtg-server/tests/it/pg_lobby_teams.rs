//! Goal: teams the host sets in the lobby, whole or one seat at a time, are stored when set, so a restart before the start keeps them and a cleared assignment stays cleared, and `/api/me/games` names the team of the user's seat and of every other seat (none without teams).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_server::{ServerConfig, migrate};
use net::lobby::body;
use net::{Browser, PASSWORD, TestServer, bot_config, get, register};
use serde_json::{Value, json};
use sqlx::PgPool;

fn config() -> ServerConfig {
    let mut config = bot_config();
    config.auth.limits.register_per_ip = 10_000;
    config.auth.limits.lobby_requests_per_ip = 100_000;
    config
}

async fn restart(server: TestServer, pool: &PgPool) -> TestServer {
    let mut server = server;
    server.shutdown();
    server.finished().await.unwrap();
    drop(server);
    TestServer::start_on(pool.clone(), config()).await
}

async fn teams_of(server: &TestServer, game: &str) -> Value {
    let info = get(server.addr, &format!("/api/games/{game}")).await;
    assert_eq!(info.status, 200, "{}", info.text());
    info.json()["house_rules"]["teams"].clone()
}

/// The (own team, other seats' teams) of `game` in the user's list.
async fn listed(server: &TestServer, user: &Browser, game: &str) -> (Value, Vec<Value>) {
    let games = user.games(server.addr).await;
    let entry = games.iter().find(|g| g["game"] == game).unwrap();
    let others = entry["others"]
        .as_array()
        .unwrap()
        .iter()
        .map(|seat| seat["team"].clone())
        .collect();
    (entry["team"].clone(), others)
}

#[sqlx::test(migrations = false)]
async fn lobby_teams_survive_a_restart_and_my_games_names_each_seats_team(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let server = TestServer::start_on(pool.clone(), config()).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let (game, _) = ann
        .create_game(server.addr, &body("Ann", 4, 4, "invite"))
        .await;
    assert_eq!(
        listed(&server, &ann, &game).await,
        (Value::Null, vec![Value::Null; 3])
    );

    let giant = json!({"mode": "two_headed_giant", "seats": [0, 0, 1, 1]});
    let set = ann
        .put(
            server.addr,
            &format!("/api/games/{game}/teams"),
            &json!({ "teams": giant }).to_string(),
        )
        .await;
    assert_eq!(set.status, 200, "{}", set.text());
    // One seat moved on its own is stored too.
    let moved = ann
        .put(
            server.addr,
            &format!("/api/games/{game}/seats/3/team"),
            &json!({ "team": 0 }).to_string(),
        )
        .await;
    assert_eq!(moved.status, 200, "{}", moved.text());
    let chosen = json!({"mode": "two_headed_giant", "seats": [0, 0, 1, 0]});

    let server = restart(server, &pool).await;
    assert_eq!(teams_of(&server, &game).await, chosen);
    assert_eq!(
        listed(&server, &ann, &game).await,
        (json!(0), vec![json!(0), json!(1), json!(0)])
    );

    let cleared = ann
        .put(
            server.addr,
            &format!("/api/games/{game}/teams"),
            &json!({ "teams": null }).to_string(),
        )
        .await;
    assert_eq!(cleared.status, 200, "{}", cleared.text());
    let mut server = restart(server, &pool).await;
    assert_eq!(teams_of(&server, &game).await, Value::Null);
    assert_eq!(
        listed(&server, &ann, &game).await,
        (Value::Null, vec![Value::Null; 3])
    );
    server.shutdown();
    server.finished().await.unwrap();
}
