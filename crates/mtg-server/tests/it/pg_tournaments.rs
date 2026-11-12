//! Goal: tournaments work end to end on Postgres: an organiser creates one, players sign up by invite,
//! each round's matches become games whose seats are bound to their players, results come from the
//! game records and the organiser's corrections, Swiss rounds avoid rematches and hand out byes,
//! elimination ends with one entrant, drops stop pairings, and strangers, players and guests are
//! refused what is the organiser's; a match game has no player host, and one dropped without a result (closed or
//! expired) is made again when the tournament is read; an entrant dropped mid-game concedes it;
//! a match played to its end on sockets reports its result by itself, the standings carry the tiebreakers through
//! draws, drops and corrections, an elimination bracket of three gives a bye, and a 2v2 match is one team game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use std::collections::BTreeMap;
use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_protocol::ClientMessage;
use mtg_server::{ServerConfig, migrate};
use net::WsBot;
use net::bot::Stop;
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
async fn teams_sign_up_together_and_start_only_complete(pool: PgPool) {
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
    assert_eq!(view(&server, &id, &org).await["playable"], true);
    // One team is no field, and a second team still missing its mate is not complete.
    let refused = act(&server, &id, "start", &org).await;
    assert_eq!(
        (refused.status, refused.error_code().as_str()),
        (409, "not_enough_entrants")
    );
    assert_eq!(
        join(&server, &invite, &cy, json!({"team_name": "Blue"}))
            .await
            .status,
        200
    );
    let refused = act(&server, &id, "start", &org).await;
    assert_eq!(
        (refused.status, refused.error_code().as_str()),
        (409, "not_enough_entrants")
    );
    assert_eq!(act(&server, &id, "leave", &cy).await.status, 204);
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

async fn rows(pool: &PgPool, table: &str) -> i64 {
    // `table` is always one of this file's own names.
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn bye(server: &TestServer, who: &Browser) -> Reply {
    let body = json!({"password": PASSWORD});
    call(server, "DELETE", "/api/auth/account", Some(who), Some(body)).await
}

#[sqlx::test(migrations = false)]
async fn a_deleted_account_takes_its_tournament_rows_with_it(pool: PgPool) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "Gone", "format": "commander_1v1", "structure": "swiss"}),
    )
    .await;
    assert_eq!(join(&server, &invite, &ann, json!({})).await.status, 200);
    assert_eq!(rows(&pool, "tournament_players").await, 1);
    assert_eq!(bye(&server, &ann).await.status, 204);
    assert_eq!(rows(&pool, "tournament_players").await, 0);
    assert_eq!(view(&server, &id, &org).await["entrants"], json!([]));
    assert_eq!(bye(&server, &org).await.status, 204);
    for table in ["tournaments", "tournament_entrants", "tournament_matches"] {
        assert_eq!(rows(&pool, table).await, 0, "{table}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_match_game_has_no_player_host_and_one_dropped_without_a_result_is_made_again(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "Cup", "format": "commander_1v1", "structure": "elimination"}),
    )
    .await;
    let mut players = Vec::new();
    for name in ["Pia", "Pbo"] {
        let player = register(server.addr, name, PASSWORD).await;
        assert_eq!(join(&server, &invite, &player, json!({})).await.status, 200);
        players.push((name, player));
    }
    let t = act(&server, &id, "start", &org).await.json();
    let game = round(&t, 1)[0]["game"].as_str().unwrap().to_owned();
    let first = entrant_named(&t, round(&t, 1)[0]["a"].as_u64().unwrap());

    // Seat 0 is a player like the other: neither closes, hides or splits the table.
    let host_only = [
        ("DELETE", format!("/api/games/{game}"), None),
        (
            "PATCH",
            format!("/api/games/{game}"),
            Some(json!({"visibility": "public"})),
        ),
        (
            "PUT",
            format!("/api/games/{game}/teams"),
            Some(json!({"teams": null})),
        ),
    ];
    for (_, player) in &players {
        for (method, path, body) in &host_only {
            let reply = call(&server, method, path, Some(player), body.clone()).await;
            assert_eq!(
                reply.status,
                403,
                "{method} {path} by {first}: {}",
                reply.text()
            );
        }
    }
    assert!(server.registry.handle(game.parse().unwrap()).is_some());

    // The lobby is dropped without a result (closed): the next read of the tournament makes a new game.
    server
        .registry
        .close(game.parse().unwrap(), mtg_server::registry::HOST)
        .await
        .unwrap();
    let remade = |t: &Value| round(t, 1)[0]["game"].as_str().unwrap().to_owned();
    let fresh = remade(&view(&server, &id, &org).await);
    assert_ne!(fresh, game);
    assert!(server.registry.handle(fresh.parse().unwrap()).is_some());
    assert_eq!(seat_owners(&pool, &fresh).await.len(), 2);

    // The new lobby expires: a player's read makes another, and the organiser's retry has nothing left to make.
    let later = std::time::Instant::now() + Duration::from_secs(400 * 24 * 3600);
    assert_eq!(server.registry.sweep(later).await, 1);
    let again = remade(&view(&server, &id, &players[1].1).await);
    assert!(![&game, &fresh].contains(&&again), "{again}");
    assert!(server.registry.handle(again.parse().unwrap()).is_some());
    let retried = act(&server, &id, "games", &org).await;
    assert_eq!(retried.status, 200, "{}", retried.text());
    assert_eq!(remade(&retried.json()), again);
}

/// A tournament server that also holds the cards, so players bring Commander decks to their match games.
async fn start_with_cards(pool: &PgPool) -> TestServer {
    let mut config: ServerConfig = bot_config();
    config.auth.limits.register_per_ip = 10_000;
    crate::pg_commander::start_with(pool, config).await
}

/// A one-round Swiss tournament of two players with decks, started: its organiser, id and match, and the players in
/// the order of the match (entrant a first).
async fn live_match(server: &TestServer) -> (Browser, String, Value, [(Browser, String); 2]) {
    let org = register(server.addr, "Org", PASSWORD).await;
    let (id, invite) = create(
        server,
        &org,
        json!({"name": "Live", "format": "commander_1v1", "structure": "swiss", "rounds": 1}),
    )
    .await;
    let mut players = Vec::new();
    for (name, leader) in [("Ann", "Questing Beast"), ("Bob", "Rosheen Meanderer")] {
        let player = register(server.addr, name, PASSWORD).await;
        let deck = pg_commander::store(server, &player, &pg_commander::led_by(leader)).await;
        assert_eq!(join(server, &invite, &player, json!({})).await.status, 200);
        players.push((player, deck));
    }
    let t = act(server, &id, "start", &org).await.json();
    let played = round(&t, 1)[0].clone();
    let first = entrant_named(&t, played["a"].as_u64().unwrap());
    if players[0].0.username != first {
        players.swap(0, 1);
    }
    let [a, b]: [(Browser, String); 2] = players.try_into().unwrap();
    (org, id, played, [a, b])
}

/// The match of round 1 once the tournament has a result for it.
async fn decided(server: &TestServer, id: &str, who: &Browser) -> Value {
    let mut synced = Value::Null;
    for _ in 0..200 {
        synced = round(&view(server, id, who).await, 1)[0].clone();
        if !synced["outcome"].is_null() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    synced
}

#[sqlx::test(migrations = false)]
async fn a_match_played_to_its_end_on_sockets_reports_its_result_by_itself(pool: PgPool) {
    let server = start_with_cards(&pool).await;
    let (org, id, played, [(ann, ann_deck), (bob, bob_deck)]) = live_match(&server).await;
    let game = played["game"].as_str().unwrap().to_owned();

    // Each player brings a deck to their own seat, and to no other.
    let other = pg_commander::choose(&server, &bob, &game, 0, &bob_deck).await;
    assert_eq!(other.status, 403, "{}", other.text());
    for (seat, who, deck) in [(0, &ann, &ann_deck), (1, &bob, &bob_deck)] {
        let chosen = pg_commander::choose(&server, who, &game, seat, deck).await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }

    // The seats play on sockets; the second player concedes at the opening hand.
    let mut seat_a = WsBot::connect_as_user(server.addr, &game, &ann.cookie, 0, 1).await;
    let mut seat_b = WsBot::connect_as_user(server.addr, &game, &bob.cookie, 1, 1).await;
    seat_b.pause_at = Some(1);
    let winner = tokio::spawn(async move {
        seat_a.ready().await;
        seat_a.finish().await
    });
    seat_b.ready().await;
    assert_eq!(seat_b.play().await, Stop::Paused);
    seat_b.client.send(&ClientMessage::Concede).await;
    assert_eq!(
        winner.await.unwrap().result,
        GameResult::Winner(PlayerId::from_index(0))
    );

    // The tournament reads the result from the game record; nobody enters it.
    let synced = decided(&server, &id, &bob).await;
    assert_eq!(
        (synced["outcome"].as_str(), synced["corrected"].as_bool()),
        (Some("a"), Some(false))
    );
    let t = act(&server, &id, "next", &org).await.json();
    assert_eq!(t["status"], "finished");
    assert_eq!(t["standings"][0]["entrant"], played["a"]);
    assert_eq!(t["standings"][0]["points"], 3);
    assert_eq!(t["standings"][1]["losses"], 1);
}

#[sqlx::test(migrations = false)]
async fn a_player_who_drops_mid_game_concedes_it_and_the_game_records_the_loss(pool: PgPool) {
    dropped_mid_game(pool, false).await;
}

#[sqlx::test(migrations = false)]
async fn an_entrant_the_organiser_drops_mid_game_concedes_it_and_the_game_records_the_loss(
    pool: PgPool,
) {
    dropped_mid_game(pool, true).await;
}

/// The second entrant of a running match game drops out of the tournament, by itself or by the organiser.
async fn dropped_mid_game(pool: PgPool, by_organiser: bool) {
    let server = start_with_cards(&pool).await;
    let (org, id, played, [(ann, ann_deck), (bob, bob_deck)]) = live_match(&server).await;
    let game = played["game"].as_str().unwrap().to_owned();
    for (seat, who, deck) in [(0, &ann, &ann_deck), (1, &bob, &bob_deck)] {
        let chosen = pg_commander::choose(&server, who, &game, seat, deck).await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    // Both seats play; the second stops at its first decision and never answers again.
    let mut seat_a = WsBot::connect_as_user(server.addr, &game, &ann.cookie, 0, 1).await;
    let mut seat_b = WsBot::connect_as_user(server.addr, &game, &bob.cookie, 1, 1).await;
    seat_b.pause_at = Some(1);
    let winner = tokio::spawn(async move {
        seat_a.ready().await;
        seat_a.finish().await
    });
    seat_b.ready().await;
    assert_eq!(seat_b.play().await, Stop::Paused);

    let dropped = if by_organiser {
        let path = format!("/api/tournaments/{id}/entrants/{}/drop", played["b"]);
        call(&server, "POST", &path, Some(&org), None).await
    } else {
        act(&server, &id, "drop", &bob).await
    };
    assert_eq!(dropped.status, 200, "{}", dropped.text());

    // The game ends as the dropped player's concession, and the match is the game's result, not a ruling.
    assert_eq!(
        winner.await.unwrap().result,
        GameResult::Winner(PlayerId::from_index(0))
    );
    let synced = decided(&server, &id, &ann).await;
    assert_eq!(
        (synced["outcome"].as_str(), synced["corrected"].as_bool()),
        (Some("a"), Some(false))
    );
}

/// Ends a match's game: the entrant with the lower number wins.
async fn settle(pool: &PgPool, played: &Value) {
    let a_wins = played["a"].as_u64() < played["b"].as_u64();
    let seat = usize::from(!a_wins);
    let game = played["game"].as_str().unwrap();
    finish_game(pool, game, GameResult::Winner(PlayerId::from_index(seat))).await;
}

/// Checks the standings against the rounds: the records, the tiebreakers (opponents' match-win fraction, own and
/// opponents' game-win fraction, each at least 1/3) and their order.
fn check_standings(t: &Value) {
    let mut record: BTreeMap<u64, (u32, u32, Vec<u64>)> = BTreeMap::new();
    for played in t["rounds_played"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r["matches"].as_array().unwrap())
    {
        let a = played["a"].as_u64().unwrap();
        let Some(b) = played["b"].as_u64() else {
            let entry = record.entry(a).or_default();
            entry.0 += 3;
            entry.1 += 1;
            continue;
        };
        let Some(outcome) = played["outcome"].as_str() else {
            continue;
        };
        let (pa, pb) = match outcome {
            "a" => (3, 0),
            "b" => (0, 3),
            _ => (1, 1),
        };
        for (me, them, points) in [(a, b, pa), (b, a, pb)] {
            let entry = record.entry(me).or_default();
            entry.0 += points;
            entry.1 += 1;
            entry.2.push(them);
        }
    }
    let fraction = |entrant: u64| {
        let (points, played, _) = &record[&entrant];
        (f64::from(*points) / f64::from(3 * played)).max(1.0 / 3.0)
    };
    let standings = t["standings"].as_array().unwrap();
    assert_eq!(standings.len(), t["entrants"].as_array().unwrap().len());
    let mut previous: Option<(u32, f64, f64, f64)> = None;
    for (at, row) in standings.iter().enumerate() {
        let entrant = row["entrant"].as_u64().unwrap();
        let (points, _, opponents) = &record[&entrant];
        let omw = opponents.iter().map(|o| fraction(*o)).sum::<f64>()
            / f64::from(u32::try_from(opponents.len()).unwrap());
        let near = |field: &str, value: f64| {
            assert!(
                (row[field].as_f64().unwrap() - value).abs() < 1e-3,
                "entrant {entrant}: {field} {} is not {value}",
                row[field]
            );
        };
        assert_eq!(row["rank"], at as u64 + 1);
        assert_eq!(
            row["points"].as_u64(),
            Some(u64::from(*points)),
            "entrant {entrant}"
        );
        near("opponents_match_win", omw);
        near("game_win", fraction(entrant));
        near("opponents_game_win", omw);
        let key = (*points, omw, fraction(entrant), omw);
        if let Some(before) = previous {
            assert!(
                before.0 > key.0 || (before.0 == key.0 && (before.1, before.2) >= (key.1, key.2)),
                "rank {} is ahead of rank {}",
                at + 1,
                at
            );
        }
        previous = Some(key);
    }
}

#[sqlx::test(migrations = false)]
async fn swiss_standings_follow_points_and_tiebreakers_through_draws_drops_and_corrections(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "League", "format": "commander_1v1", "structure": "swiss", "rounds": 3}),
    )
    .await;
    let mut players = Vec::new();
    for name in ["Pam", "Pbo", "Pcy", "Pdo"] {
        let player = register(server.addr, name, PASSWORD).await;
        assert_eq!(join(&server, &invite, &player, json!({})).await.status, 200);
        players.push(player);
    }
    let t = act(&server, &id, "start", &org).await.json();
    let first = round(&t, 1);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|m| m["game"].is_string()));
    for played in &first {
        settle(&pool, played).await;
    }
    let winners: Vec<u64> = first
        .iter()
        .map(|m| m["a"].as_u64().min(m["b"].as_u64()).unwrap())
        .collect();

    // Round 2 pairs the winners together and the losers together.
    let t = act(&server, &id, "next", &org).await.json();
    check_standings(&t);
    let second = round(&t, 2);
    for played in &second {
        let (a, b) = (played["a"].as_u64().unwrap(), played["b"].as_u64().unwrap());
        assert_eq!(winners.contains(&a), winners.contains(&b), "{played}");
    }
    // One match the organiser calls a draw before its game ends; a player may not.
    let path = format!("/api/tournaments/{id}/matches/2/{}", second[0]["table"]);
    let draw = Some(json!({"outcome": "draw"}));
    let refused = call(&server, "PUT", &path, Some(&players[0]), draw.clone()).await;
    assert_eq!(refused.status, 403);
    assert_eq!(
        call(&server, "PUT", &path, Some(&org), draw).await.status,
        200
    );
    settle(&pool, &second[1]).await;

    // Round 3: the organiser drops an entrant mid-round and its opponent takes the match; a player drops nobody else.
    let t = act(&server, &id, "next", &org).await.json();
    check_standings(&t);
    let third = round(&t, 3);
    let gone = third[0]["a"].as_u64().unwrap();
    let path = format!("/api/tournaments/{id}/entrants/{gone}/drop");
    let refused = call(&server, "POST", &path, Some(&players[0]), None).await;
    assert_eq!(refused.status, 403);
    let dropped = call(&server, "POST", &path, Some(&org), None).await;
    assert_eq!(dropped.status, 200, "{}", dropped.text());
    let t = dropped.json();
    assert_eq!(
        (
            round(&t, 3)[0]["outcome"].as_str(),
            round(&t, 3)[0]["corrected"].as_bool()
        ),
        (Some("b"), Some(true))
    );
    let entrant = |t: &Value, n: u64| {
        t["entrants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["entrant"] == n)
            .unwrap()
            .clone()
    };
    assert_eq!(entrant(&t, gone)["dropped"], true);
    settle(&pool, &third[1]).await;

    // The last round ends the tournament with the final standings.
    let t = act(&server, &id, "next", &org).await.json();
    assert_eq!(t["status"], "finished");
    check_standings(&t);
    let points: u64 = t["standings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["points"].as_u64().unwrap())
        .sum();
    assert_eq!(points, 3 * 5 + 2, "five decided matches and one draw");
}

#[sqlx::test(migrations = false)]
async fn an_elimination_bracket_of_three_gives_a_bye_and_crowns_the_last_winner(pool: PgPool) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "Cup", "format": "commander_1v1", "structure": "elimination"}),
    )
    .await;
    for name in ["Pam", "Pbo", "Pcy"] {
        let player = register(server.addr, name, PASSWORD).await;
        assert_eq!(join(&server, &invite, &player, json!({})).await.status, 200);
    }
    let t = act(&server, &id, "start", &org).await.json();
    assert_eq!(t["rounds"], 2);
    let first = round(&t, 1);
    let bye = first.iter().find(|m| m["b"].is_null()).unwrap().clone();
    assert!(bye["game"].is_null(), "a bye has no game");
    let played = first.iter().find(|m| !m["b"].is_null()).unwrap().clone();
    settle(&pool, &played).await;
    let t = act(&server, &id, "next", &org).await.json();
    let last = round(&t, 2);
    assert_eq!(last.len(), 1);
    let finalists = [last[0]["a"].clone(), last[0]["b"].clone()];
    let survivor = json!(played["a"].as_u64().min(played["b"].as_u64()));
    assert!(finalists.contains(&bye["a"]) && finalists.contains(&survivor));
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
async fn a_team_tournament_seats_both_teams_in_one_team_game(pool: PgPool) {
    let server = start(&pool).await;
    let org = register(server.addr, "Org", PASSWORD).await;
    let (id, invite) = create(
        &server,
        &org,
        json!({"name": "Pairs", "format": "commander_2v2", "structure": "swiss", "rounds": 1}),
    )
    .await;
    for (captain, mate, team) in [("Ann", "Alf", "Red"), ("Bob", "Bea", "Blue")] {
        let captain = register(server.addr, captain, PASSWORD).await;
        let made = join(&server, &invite, &captain, json!({"team_name": team})).await;
        let number = made.json()["entrants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == team)
            .unwrap()["entrant"]
            .clone();
        let mate = register(server.addr, mate, PASSWORD).await;
        let joined = join(&server, &invite, &mate, json!({"team": number})).await;
        assert_eq!(joined.status, 200, "{}", joined.text());
    }
    assert_eq!(view(&server, &id, &org).await["playable"], true);
    let started = act(&server, &id, "start", &org).await;
    assert_eq!(started.status, 200, "{}", started.text());
    let t = started.json();
    let played = round(&t, 1)[0].clone();
    let game = played["game"].as_str().unwrap().to_owned();
    let players = |side: &str| {
        t["entrants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["entrant"] == played[side])
            .unwrap()["players"]
            .clone()
    };
    let (a, b) = (players("a"), players("b"));
    let owners: Vec<Value> = seat_owners(&pool, &game)
        .await
        .into_iter()
        .map(|name| json!(name))
        .collect();
    assert_eq!(
        owners,
        [a[0].clone(), b[0].clone(), a[1].clone(), b[1].clone()]
    );
    let lookup = net::get(server.addr, &format!("/api/games/{game}"))
        .await
        .json();
    assert_eq!(
        lookup["house_rules"]["teams"],
        json!({"mode": "teams", "seats": [0, 1, 0, 1]})
    );
    finish_game(&pool, &game, GameResult::Team(1)).await;
    let t = view(&server, &id, &org).await;
    assert_eq!(round(&t, 1)[0]["outcome"], "b");
}
