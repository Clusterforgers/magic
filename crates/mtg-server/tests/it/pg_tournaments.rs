//! Goal: tournaments work end to end on Postgres: an organiser creates one, players sign up by invite,
//! each round's matches become games whose seats are bound to their players, results come from the
//! game records and the organiser's corrections, Swiss rounds avoid rematches and hand out byes,
//! elimination ends with one entrant, drops stop pairings, and strangers, players and guests are
//! refused what is the organiser's.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_server::{ServerConfig, migrate};
use net::{Browser, PASSWORD, Reply, TestServer, bot_config, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    let mut config: ServerConfig = bot_config();
    config.auth.limits.register_per_ip = 10_000;
    TestServer::start_on(pool.clone(), config).await
}

async fn call(
    server: &TestServer,
    method: &str,
    path: &str,
    who: Option<&Browser>,
    body: Option<Value>,
) -> Reply {
    let mut headers: Vec<(&str, &str)> = Vec::new();
    if let Some(browser) = who {
        headers.push(("Cookie", browser.cookie.as_str()));
    }
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    request(server.addr, method, path, &headers, body.as_bytes()).await
}

/// Creates a tournament as `organiser`; answers its id and invite.
async fn create(server: &TestServer, organiser: &Browser, body: Value) -> (String, String) {
    let reply = call(
        server,
        "POST",
        "/api/tournaments",
        Some(organiser),
        Some(body),
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let json = reply.json();
    (
        json["id"].as_str().unwrap().to_owned(),
        json["invite"].as_str().unwrap().to_owned(),
    )
}

async fn join(server: &TestServer, invite: &str, who: &Browser, body: Value) -> Reply {
    let path = format!("/api/tournaments/invite/{invite}/join");
    call(server, "POST", &path, Some(who), Some(body)).await
}

async fn view(server: &TestServer, id: &str, who: &Browser) -> Value {
    let reply = call(
        server,
        "GET",
        &format!("/api/tournaments/{id}"),
        Some(who),
        None,
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

async fn act(server: &TestServer, id: &str, what: &str, who: &Browser) -> Reply {
    call(
        server,
        "POST",
        &format!("/api/tournaments/{id}/{what}"),
        Some(who),
        None,
    )
    .await
}

/// Ends a match's game in the records the way the recorder does.
async fn finish_game(pool: &PgPool, game: &str, result: GameResult) {
    sqlx::query("UPDATE games SET status = 'finished', result = $2 WHERE id = $1::uuid")
        .bind(game)
        .bind(serde_json::to_value(result).unwrap())
        .execute(pool)
        .await
        .unwrap();
}

/// The usernames bound to a game's seats, in seat order.
async fn seat_owners(pool: &PgPool, game: &str) -> Vec<Option<String>> {
    sqlx::query_scalar(
        "SELECT u.username FROM seats s LEFT JOIN users u ON u.id = s.user_id
         WHERE s.game_id = $1::uuid ORDER BY s.seat",
    )
    .bind(game)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn entrant_named(tournament: &Value, entrant: u64) -> String {
    tournament["entrants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["entrant"] == entrant)
        .map(|e| e["name"].as_str().unwrap().to_owned())
        .unwrap()
}

fn round(tournament: &Value, n: usize) -> Vec<Value> {
    tournament["rounds_played"][n - 1]["matches"]
        .as_array()
        .unwrap()
        .clone()
}

#[sqlx::test(migrations = false)]
async fn a_swiss_tournament_runs_from_signup_to_standings(pool: PgPool) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let stranger = register(server.addr, "Stranger", PASSWORD).await;

    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "Friday", "format": "commander_1v1", "structure": "swiss", "rounds": 3}),
    )
    .await;
    // The invite shows what one signs up for; a wrong code says nothing.
    let preview = call(
        &server,
        "GET",
        &format!("/api/tournaments/invite/{invite}"),
        Some(&ann),
        None,
    )
    .await;
    assert_eq!(preview.json()["name"], "Friday");
    assert_eq!(preview.json()["joined"], false);
    let wrong = call(
        &server,
        "GET",
        "/api/tournaments/invite/AAAAAAAA",
        Some(&ann),
        None,
    )
    .await;
    assert_eq!(wrong.status, 404);

    for who in [&org, &ann, &bo] {
        let reply = join(&server, &invite, who, json!({})).await;
        assert_eq!(reply.status, 200, "{}", reply.text());
    }
    let again = join(&server, &invite, &ann, json!({})).await;
    assert_eq!(
        (again.status, again.error_code().as_str()),
        (409, "already_joined")
    );

    // Only the organiser and players see the tournament; only the organiser runs it.
    let hidden = call(
        &server,
        "GET",
        &format!("/api/tournaments/{id}"),
        Some(&stranger),
        None,
    )
    .await;
    assert_eq!(hidden.status, 404);
    let guest = call(
        &server,
        "GET",
        &format!("/api/tournaments/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(guest.status, 401);
    let by_player = act(&server, &id, "start", &ann).await;
    assert_eq!(
        (by_player.status, by_player.error_code().as_str()),
        (403, "forbidden")
    );
    let by_stranger = act(&server, &id, "start", &stranger).await;
    assert_eq!(by_stranger.status, 404);
    let player_view = view(&server, &id, &ann).await;
    assert_eq!(player_view["invite"], Value::Null);
    assert_eq!(view(&server, &id, &org).await["invite"], invite.as_str());

    let started = act(&server, &id, "start", &org).await;
    assert_eq!(started.status, 200, "{}", started.text());
    let late = join(&server, &invite, &stranger, json!({})).await;
    assert_eq!((late.status, late.error_code().as_str()), (409, "started"));

    // Three entrants: one match and one bye, the match's game bound to its two players.
    let t = started.json();
    assert_eq!(t["status"], "running");
    assert_eq!(t["rounds"], 3);
    let first = round(&t, 1);
    assert_eq!(first.len(), 2);
    let played = first.iter().find(|m| !m["b"].is_null()).unwrap().clone();
    let bye = first.iter().find(|m| m["b"].is_null()).unwrap().clone();
    assert!(bye["game"].is_null());
    let game = played["game"].as_str().unwrap().to_owned();
    let a = entrant_named(&t, played["a"].as_u64().unwrap());
    let b = entrant_named(&t, played["b"].as_u64().unwrap());
    assert_eq!(
        seat_owners(&pool, &game).await,
        vec![Some(a.clone()), Some(b.clone())]
    );

    // The round cannot move on while the game runs; its result comes from the game record.
    let early = act(&server, &id, "next", &org).await;
    assert_eq!(
        (early.status, early.error_code().as_str()),
        (409, "undecided")
    );
    finish_game(&pool, &game, GameResult::Winner(PlayerId::from_index(1))).await;
    let t = view(&server, &id, &ann).await;
    let synced = round(&t, 1)
        .into_iter()
        .find(|m| m["game"] == game.as_str())
        .unwrap();
    assert_eq!(synced["outcome"], "b");
    assert_eq!(synced["corrected"], false);
    // The winner and the bye lead with 3 points each.
    let points: Vec<u64> = t["standings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["points"].as_u64().unwrap())
        .collect();
    assert_eq!(points, vec![3, 3, 0]);

    // Round 2 pairs the bye with somebody new and gives the bye to someone who had none.
    let t = act(&server, &id, "next", &org).await.json();
    let second = round(&t, 2);
    let bye_two = second.iter().find(|m| m["b"].is_null()).unwrap();
    assert_ne!(bye_two["a"], bye["a"]);
    let match_two = second.iter().find(|m| !m["b"].is_null()).unwrap().clone();
    let pair = |m: &Value| {
        let mut p = [m["a"].as_u64().unwrap(), m["b"].as_u64().unwrap()];
        p.sort_unstable();
        p
    };
    assert_ne!(pair(&match_two), pair(&played));

    // The organiser corrects a result; a game's later result does not overwrite it.
    let table = match_two["table"].as_u64().unwrap();
    let path = format!("/api/tournaments/{id}/matches/2/{table}");
    let set = call(
        &server,
        "PUT",
        &path,
        Some(&org),
        Some(json!({"outcome": "draw"})),
    )
    .await;
    assert_eq!(set.status, 200, "{}", set.text());
    let by_player = call(
        &server,
        "PUT",
        &path,
        Some(&ann),
        Some(json!({"outcome": "a"})),
    )
    .await;
    assert_eq!(by_player.status, 403);
    let game_two = match_two["game"].as_str().unwrap().to_owned();
    finish_game(
        &pool,
        &game_two,
        GameResult::Winner(PlayerId::from_index(0)),
    )
    .await;
    let t = view(&server, &id, &org).await;
    let kept = round(&t, 2)
        .into_iter()
        .find(|m| m["table"] == table)
        .unwrap();
    assert_eq!(
        (kept["outcome"].as_str(), kept["corrected"].as_bool()),
        (Some("draw"), Some(true))
    );

    // A dropped player is paired no more; the third round is the last.
    let dropped = act(&server, &id, "drop", &bo).await;
    assert_eq!(dropped.status, 200, "{}", dropped.text());
    let t = act(&server, &id, "next", &org).await.json();
    let third = round(&t, 3);
    let bo_entrant = t["entrants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "Bob")
        .unwrap()["entrant"]
        .clone();
    assert!(
        third
            .iter()
            .all(|m| m["a"] != bo_entrant && m["b"] != bo_entrant)
    );
    for m in &third {
        if let Some(game) = m["game"].as_str() {
            finish_game(&pool, game, GameResult::Winner(PlayerId::from_index(0))).await;
        }
    }
    let t = act(&server, &id, "next", &org).await.json();
    assert_eq!(t["status"], "finished");
    assert_eq!(t["standings"].as_array().unwrap().len(), 3);
    let after = act(&server, &id, "next", &org).await;
    assert_eq!(
        (after.status, after.error_code().as_str()),
        (409, "not_running")
    );

    // The list shows it to its organiser and players.
    let listed = call(&server, "GET", "/api/tournaments", Some(&ann), None)
        .await
        .json();
    assert_eq!(listed["tournaments"][0]["id"], id.as_str());
    assert_eq!(listed["tournaments"][0]["organiser"], false);
    let none = call(&server, "GET", "/api/tournaments", Some(&stranger), None)
        .await
        .json();
    assert_eq!(none["tournaments"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = false)]
async fn an_elimination_bracket_needs_a_winner_of_every_match(pool: PgPool) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "Cup", "format": "commander_1v1", "structure": "elimination"}),
    )
    .await;
    let mut players = Vec::new();
    for name in ["Pia", "Pbo", "Pcy", "Pdo"] {
        let player = register(server.addr, name, PASSWORD).await;
        assert_eq!(join(&server, &invite, &player, json!({})).await.status, 200);
        players.push(player);
    }
    let t = act(&server, &id, "start", &org).await.json();
    assert_eq!(t["rounds"], 2);
    let first = round(&t, 1);
    assert_eq!(first.len(), 2);
    // A drawn game must be settled by the organiser before the bracket goes on.
    finish_game(&pool, first[0]["game"].as_str().unwrap(), GameResult::Draw).await;
    finish_game(
        &pool,
        first[1]["game"].as_str().unwrap(),
        GameResult::Winner(PlayerId::from_index(0)),
    )
    .await;
    let drawn = act(&server, &id, "next", &org).await;
    assert_eq!(
        (drawn.status, drawn.error_code().as_str()),
        (409, "undecided")
    );
    let path = format!("/api/tournaments/{id}/matches/1/1");
    let set = call(
        &server,
        "PUT",
        &path,
        Some(&org),
        Some(json!({"outcome": "b"})),
    )
    .await;
    assert_eq!(set.status, 200);
    let t = act(&server, &id, "next", &org).await.json();
    let last = round(&t, 2);
    assert_eq!(last.len(), 1);
    assert_eq!(last[0]["a"], first[0]["b"]);
    assert_eq!(last[0]["b"], first[1]["a"]);
    // An earlier elimination round is final.
    let old = call(
        &server,
        "PUT",
        &path,
        Some(&org),
        Some(json!({"outcome": "a"})),
    )
    .await;
    assert_eq!(old.status, 409);
    finish_game(
        &pool,
        last[0]["game"].as_str().unwrap(),
        GameResult::Winner(PlayerId::from_index(1)),
    )
    .await;
    let t = act(&server, &id, "next", &org).await.json();
    assert_eq!(t["status"], "finished");
    assert_eq!(t["standings"][0]["entrant"], last[0]["b"]);
}

#[sqlx::test(migrations = false)]
async fn teams_sign_up_together_and_wait_for_team_games(pool: PgPool) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "Pairs", "format": "commander_2v2", "structure": "swiss"}),
    )
    .await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let al = register(server.addr, "Alf", PASSWORD).await;
    let cy = register(server.addr, "Cyd", PASSWORD).await;
    let made = join(&server, &invite, &ann, json!({"team_name": "Red"}))
        .await
        .json();
    let team = made["entrants"][0]["entrant"].as_u64().unwrap();
    assert_eq!(made["entrants"][0]["name"], "Red");
    let joined = join(&server, &invite, &al, json!({"team": team}))
        .await
        .json();
    assert_eq!(joined["entrants"][0]["players"], json!(["Ann", "Alf"]));
    let full = join(&server, &invite, &cy, json!({"team": team})).await;
    assert_eq!((full.status, full.error_code().as_str()), (409, "full"));
    let missing = join(&server, &invite, &cy, json!({"team": 9})).await;
    assert_eq!(missing.status, 404);
    assert_eq!(view(&server, &id, &org).await["playable"], false);
    let refused = act(&server, &id, "start", &org).await;
    assert_eq!(
        (refused.status, refused.error_code().as_str()),
        (409, "format_unavailable")
    );
    // Leaving before the start empties a team, which goes with its last player.
    assert_eq!(act(&server, &id, "leave", &ann).await.status, 204);
    assert_eq!(act(&server, &id, "leave", &al).await.status, 204);
    assert_eq!(view(&server, &id, &org).await["entrants"], json!([]));
}

#[sqlx::test(migrations = false)]
async fn bodies_and_paths_are_checked(pool: PgPool) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    for body in [
        json!({"name": "", "format": "commander_1v1", "structure": "swiss"}),
        json!({"name": "x", "format": "commander_1v1", "structure": "swiss", "rounds": 0}),
        json!({"name": "x", "format": "commander_1v1", "structure": "swiss", "rounds": 16}),
        json!({"name": "x", "format": "normal", "structure": "swiss"}),
        json!({"name": "x", "format": "commander_1v1", "structure": "swiss", "extra": 1}),
    ] {
        let reply = call(&server, "POST", "/api/tournaments", Some(&org), Some(body)).await;
        assert_eq!(reply.status, 400, "{}", reply.text());
    }
    let guest = call(
        &server,
        "POST",
        "/api/tournaments",
        None,
        Some(json!({"name": "x", "format": "commander_1v1", "structure": "swiss"})),
    )
    .await;
    assert_eq!(guest.status, 401);
    for path in [
        "/api/tournaments/not-a-uuid",
        "/api/tournaments/00000000-0000-0000-0000-000000000000",
    ] {
        assert_eq!(
            call(&server, "GET", path, Some(&org), None).await.status,
            404
        );
    }
    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "Solo", "format": "commander_1v1", "structure": "swiss"}),
    )
    .await;
    join(&server, &invite, &org, json!({})).await;
    let alone = act(&server, &id, "start", &org).await;
    assert_eq!(
        (alone.status, alone.error_code().as_str()),
        (409, "not_enough_entrants")
    );
    let removed = call(
        &server,
        "DELETE",
        &format!("/api/tournaments/{id}"),
        Some(&org),
        None,
    )
    .await;
    assert_eq!(removed.status, 204);
    assert_eq!(
        call(
            &server,
            "GET",
            &format!("/api/tournaments/{id}"),
            Some(&org),
            None
        )
        .await
        .status,
        404
    );
}

#[tokio::test]
async fn a_server_without_a_database_keeps_no_tournaments() {
    let server = TestServer::start(bot_config()).await;
    let reply = call(&server, "GET", "/api/tournaments", None, None).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (503, "accounts_unavailable")
    );
}
