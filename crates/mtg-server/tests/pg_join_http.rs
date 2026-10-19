//! Goal: on Postgres a signed-in visitor joins a public game by its id or any game by its code in one request with no name, the answer names the game (a code is the only way to learn the id of an invite game), the seat is bound to the account and opens through the session alone, a repeat join returns the account's own seat without a token, leaving frees the account to join again, the host acts through the session as well as a token, a stale session is told to sign in again, and a claim or a release the database refuses is a 503 that changes nothing at the table.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use net::lobby::{self, Made, call, join, join_code};
use net::{Browser, PASSWORD, Reply, TestServer, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A migrated database behind a running server that does not count wrong codes or list requests.
async fn start(pool: &PgPool) -> TestServer {
    mtg_server::migrate(pool).await.unwrap();
    let mut config = bot_config();
    config.auth.limits.code_misses_per_ip = 100_000;
    config.auth.limits.lobby_requests_per_ip = 100_000;
    TestServer::start_on(pool.clone(), config).await
}

/// A game made by a signed-in creator: `seats` seats, the first held and bound to the creator.
async fn make_as(server: &TestServer, host: &Browser, seats: usize, visibility: &str) -> Made {
    let body = lobby::body(&host.username, seats, 1, visibility).to_string();
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

async fn names(server: &TestServer, game: &str) -> Vec<String> {
    let reply = call(server.addr, "GET", &format!("/api/games/{game}"), None, &[]).await;
    reply.json()["seats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|seat| seat["name"].as_str().unwrap().to_owned())
        .collect()
}

/// The browser's request with its session cookie and a JSON body.
async fn as_user(
    server: &TestServer,
    who: &Browser,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Reply {
    let text = body.map(Value::to_string).unwrap_or_default();
    who.send(server.addr, method, path, &text, &[]).await
}

#[sqlx::test(migrations = false)]
async fn a_signed_in_visitor_joins_by_the_id_in_one_request_and_the_seat_is_theirs(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "public").await;

    let reply = as_user(
        &server,
        &bo,
        "POST",
        &format!("/api/games/{}/join", game.id),
        Some(&json!({})),
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let body = reply.json();
    assert_eq!(body["game"], game.id);
    assert_eq!(body["seat"], 1);
    assert!(body["token"].is_string());
    // The seat is named after the account and belongs to it.
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bob", ""]);
    let mine = bo.games(server.addr).await;
    assert_eq!(mine.len(), 1);
    assert_eq!(
        (mine[0]["game"].as_str(), mine[0]["seat"].as_u64()),
        (Some(game.id.as_str()), Some(1))
    );
    assert_eq!(mine[0]["status"], "lobby");

    // The session alone opens the seat's socket; no token is needed.
    let path = format!("/ws/{}", game.id);
    let mut socket = net::open(server.addr, &path, &[("Cookie", &bo.cookie)]).await;
    assert_eq!(socket.welcome(&game.id).await.index(), 1);
}

#[sqlx::test(migrations = false)]
async fn a_signed_in_visitor_joins_by_a_code_and_may_choose_the_name(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let cy = register(server.addr, "Cyd", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "invite").await;

    let by_id = as_user(
        &server,
        &bo,
        "POST",
        &format!("/api/games/{}/join", game.id),
        Some(&json!({})),
    )
    .await;
    assert_eq!(by_id.status, 404, "an invite game is not reached by its id");
    assert!(!by_id.text().contains(&game.id), "{}", by_id.text());
    let reply = as_user(
        &server,
        &bo,
        "POST",
        "/api/join",
        Some(&json!({ "code": game.code })),
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.json()["game"], game.id, "the code names the game");
    assert_eq!(reply.json()["seat"], 1);
    let reply = as_user(
        &server,
        &cy,
        "POST",
        "/api/join",
        Some(&json!({ "code": game.code.to_lowercase(), "name": "Cyrus" })),
    )
    .await;
    assert_eq!(reply.json()["game"], game.id);
    assert_eq!(reply.json()["seat"], 2);
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bob", "Cyrus"]);
    assert_eq!(cy.games(server.addr).await[0]["seat"], 2);
}

#[sqlx::test(migrations = false)]
async fn a_name_a_signed_in_visitor_gives_is_judged_like_a_guests(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let game = make_as(&server, &ann, 2, "public").await;
    for name in ["", "  ", &"n".repeat(33), "A\nB"] {
        let reply = as_user(
            &server,
            &bo,
            "POST",
            &format!("/api/games/{}/join", game.id),
            Some(&json!({ "name": name })),
        )
        .await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (400, "invalid_name"),
            "{name:?}"
        );
    }
    assert_eq!(names(&server, &game.id).await, ["Ann", ""]);
}

#[sqlx::test(migrations = false)]
async fn a_repeat_join_returns_the_accounts_own_seat_without_a_token(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "public").await;
    let path = format!("/api/games/{}/join", game.id);

    let first = as_user(&server, &bo, "POST", &path, Some(&json!({}))).await;
    assert!(first.json()["token"].is_string());
    for _ in 0..2 {
        let again = as_user(&server, &bo, "POST", &path, Some(&json!({}))).await;
        assert_eq!(again.status, 200, "{}", again.text());
        assert_eq!(
            again.json(),
            json!({ "game": game.id, "seat": 1, "token": null })
        );
    }
    let by_code = as_user(
        &server,
        &bo,
        "POST",
        "/api/join",
        Some(&json!({ "code": game.code })),
    )
    .await;
    assert_eq!(
        by_code.json(),
        json!({ "game": game.id, "seat": 1, "token": null })
    );
    // The host, who holds seat 0, is handed that seat back too.
    let host = as_user(&server, &ann, "POST", &path, Some(&json!({}))).await;
    assert_eq!(
        host.json(),
        json!({ "game": game.id, "seat": 0, "token": null })
    );
    // No second seat was taken.
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bob", ""]);
}

#[sqlx::test(migrations = false)]
async fn a_guest_and_accounts_share_a_table_until_it_is_full(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let di = register(server.addr, "Dee", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "public").await;
    let path = format!("/api/games/{}/join", game.id);

    assert_eq!(
        join(server.addr, &game.id, &json!({ "name": "Cy" }), &[])
            .await
            .json()["seat"],
        1
    );
    assert_eq!(
        as_user(&server, &bo, "POST", &path, Some(&json!({})))
            .await
            .json()["seat"],
        2
    );
    let full = as_user(&server, &di, "POST", &path, Some(&json!({}))).await;
    assert_eq!((full.status, full.error_code().as_str()), (409, "full"));
    // An account that is already seated is not told the game is full.
    let again = as_user(&server, &bo, "POST", &path, Some(&json!({}))).await;
    assert_eq!(
        again.json(),
        json!({ "game": game.id, "seat": 2, "token": null })
    );
}

/// An account that sits in an invite game is taken back to it by the code and learns nothing from
/// the id: the id route is the same 404 for it as for a stranger.
#[sqlx::test(migrations = false)]
async fn an_account_in_an_invite_game_finds_it_by_the_code_and_not_by_the_id(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "invite").await;
    let by_code = json!({ "code": game.code });

    let first = as_user(&server, &bo, "POST", "/api/join", Some(&by_code)).await;
    assert_eq!(first.json()["game"], game.id);
    assert!(first.json()["token"].is_string());
    let again = as_user(&server, &bo, "POST", "/api/join", Some(&by_code)).await;
    assert_eq!(
        again.json(),
        json!({ "game": game.id, "seat": 1, "token": null })
    );

    let by_id = as_user(
        &server,
        &bo,
        "POST",
        &format!("/api/games/{}/join", game.id),
        Some(&json!({})),
    )
    .await;
    assert_eq!(
        (by_id.status, by_id.error_code().as_str()),
        (404, "not_found")
    );
    assert!(!by_id.text().contains(&game.id), "{}", by_id.text());
    // A code that is not the game's is the same 404 for an account, and names nothing.
    let wrong = as_user(
        &server,
        &bo,
        "POST",
        "/api/join",
        Some(&json!({ "code": "ABCD2345" })),
    )
    .await;
    assert_eq!(wrong.body, by_id.body);
}

#[sqlx::test(migrations = false)]
async fn a_game_that_started_turns_away_even_the_account_that_sits_in_it(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let game = make_as(&server, &ann, 2, "public").await;
    let path = format!("/api/games/{}/join", game.id);
    assert_eq!(
        as_user(&server, &bo, "POST", &path, Some(&json!({})))
            .await
            .status,
        200
    );
    lobby::start(&server.registry, &game.id, 2).await;

    for who in [&bo, &ann] {
        let reply = as_user(&server, who, "POST", &path, Some(&json!({}))).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (409, "started")
        );
    }
    let reply = as_user(
        &server,
        &bo,
        "POST",
        "/api/join",
        Some(&json!({ "code": game.code })),
    )
    .await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (409, "started")
    );
    // The seat itself is still theirs through the session.
    let mut socket = net::open(
        server.addr,
        &format!("/ws/{}", game.id),
        &[("Cookie", &bo.cookie)],
    )
    .await;
    assert_eq!(socket.welcome(&game.id).await.index(), 1);
}

#[sqlx::test(migrations = false)]
async fn leaving_by_the_session_frees_the_seat_and_the_account(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "public").await;
    let join_path = format!("/api/games/{}/join", game.id);
    let leave_path = format!("/api/games/{}/leave", game.id);
    let first = as_user(&server, &bo, "POST", &join_path, Some(&json!({}))).await;
    let old_token = first.json()["token"].as_str().unwrap().to_owned();

    let left = as_user(&server, &bo, "POST", &leave_path, None).await;
    assert!(
        left.status == 204 && left.body.is_empty(),
        "{} {}",
        left.status,
        left.text()
    );
    assert_eq!(names(&server, &game.id).await, ["Ann", "", ""]);
    assert!(
        bo.games(server.addr).await.is_empty(),
        "the game left the account's list"
    );
    // The account holds no seat now, so a leave is a stranger's.
    let twice = as_user(&server, &bo, "POST", &leave_path, None).await;
    assert_eq!(
        (twice.status, twice.error_code().as_str()),
        (404, "not_found")
    );
    assert_eq!(
        net::refused(server.addr, &game.id, &old_token, None)
            .await
            .0,
        404
    );
    assert_eq!(
        net::refused_at(
            server.addr,
            &format!("/ws/{}", game.id),
            &[("Cookie", &bo.cookie)]
        )
        .await
        .0,
        404
    );

    // And it may take a seat again, a new one with a new token.
    let back = as_user(&server, &bo, "POST", &join_path, Some(&json!({}))).await;
    assert_eq!(back.status, 200);
    assert_eq!(back.json()["seat"], 1);
    assert_ne!(back.json()["token"], json!(old_token));
}

#[sqlx::test(migrations = false)]
async fn the_host_acts_through_the_session_as_well_as_through_a_token(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let bo = register(server.addr, "Bob", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "public").await;
    let join_path = format!("/api/games/{}/join", game.id);
    assert_eq!(
        as_user(&server, &bo, "POST", &join_path, Some(&json!({})))
            .await
            .status,
        200
    );

    // A player's session is not the host's.
    let hide = json!({ "visibility": "invite" });
    let by_player = as_user(
        &server,
        &bo,
        "PATCH",
        &format!("/api/games/{}", game.id),
        Some(&hide),
    )
    .await;
    assert_eq!(
        (by_player.status, by_player.error_code().as_str()),
        (403, "forbidden")
    );
    let release = format!("/api/games/{}/seats/1/release", game.id);
    let by_player = as_user(&server, &bo, "POST", &release, None).await;
    assert_eq!(by_player.status, 403);

    let hidden = as_user(
        &server,
        &ann,
        "PATCH",
        &format!("/api/games/{}", game.id),
        Some(&hide),
    )
    .await;
    assert_eq!(hidden.json(), hide);
    assert_eq!(
        lobby::listed(&lobby::lobbies(server.addr, &[]).await),
        Vec::<String>::new()
    );
    let removed = as_user(&server, &ann, "POST", &release, None).await;
    assert_eq!(removed.status, 204, "{}", removed.text());
    assert_eq!(names(&server, &game.id).await, ["Ann", "", ""]);
    assert_eq!(bo.games(server.addr).await, Vec::<Value>::new());
}

#[sqlx::test(migrations = false)]
async fn without_a_session_or_a_token_a_stranger_is_told_to_sign_in_and_a_stale_cookie_is_cleared(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "public").await;
    let leave = format!("/api/games/{}/leave", game.id);

    let bare = call(server.addr, "POST", &leave, None, &[]).await;
    assert_eq!(
        (bare.status, bare.error_code().as_str()),
        (401, "unauthenticated")
    );
    assert_eq!(bare.headers_named("set-cookie"), Vec::<&str>::new());
    let stale = call(
        server.addr,
        "POST",
        &leave,
        None,
        &[(
            "Cookie",
            "mtg_session=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )],
    )
    .await;
    assert_eq!(
        (stale.status, stale.error_code().as_str()),
        (401, "unauthenticated")
    );
    let cleared = stale.headers_named("set-cookie");
    assert!(
        cleared
            .iter()
            .any(|line| line.starts_with("mtg_session=") && line.contains("Max-Age=0")),
        "{cleared:?}"
    );

    // A signed-in account that holds no seat of this game is a stranger: one 404, whatever it asks.
    let eve = register(server.addr, "Eve", PASSWORD).await;
    for (method, path, body) in [
        ("POST", leave.clone(), None),
        (
            "POST",
            format!("/api/games/{}/seats/1/release", game.id),
            None,
        ),
        (
            "PATCH",
            format!("/api/games/{}", game.id),
            Some(json!({ "visibility": "invite" })),
        ),
        (
            "POST",
            "/api/games/6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55/leave".to_owned(),
            None,
        ),
    ] {
        let reply = as_user(&server, &eve, method, &path, body.as_ref()).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (404, "not_found"),
            "{method} {path}"
        );
    }
    // A token wins over a session: Eve presenting the host's token acts as the host.
    let reply = call(
        server.addr,
        "PATCH",
        &format!("/api/games/{}", game.id),
        Some(r#"{"visibility":"invite"}"#),
        &[
            ("Cookie", &eve.cookie),
            ("Authorization", &net::lobby::bearer(game.host())),
        ],
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
}

/// A claim the database refuses leaves the table as it was.
#[sqlx::test(migrations = false)]
async fn a_claim_or_a_release_the_database_refuses_is_a_503_and_changes_nothing(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let game = make_as(&server, &ann, 3, "public").await;
    let joined = join(server.addr, &game.id, &json!({ "name": "Bo" }), &[]).await;
    let bo = joined.json()["token"].as_str().unwrap().to_owned();
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bo", ""]);

    sqlx::query("ALTER TABLE seats RENAME TO seats_away")
        .execute(&pool)
        .await
        .unwrap();
    let claim = join(server.addr, &game.id, &json!({ "name": "Cy" }), &[]).await;
    assert_eq!(
        (claim.status, claim.error_code().as_str()),
        (503, "unavailable"),
        "{}",
        claim.text()
    );
    assert!(
        !claim.text().contains("seats_away") && !claim.text().to_lowercase().contains("sql"),
        "{}",
        claim.text()
    );
    let leave = lobby::post_as(server.addr, &format!("/api/games/{}/leave", game.id), &bo).await;
    assert_eq!(
        (leave.status, leave.error_code().as_str()),
        (503, "unavailable"),
        "{}",
        leave.text()
    );
    // Nothing moved at the table: Bo still sits there and the third seat is still open.
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bo", ""]);
    let mut socket = net::connect(server.addr, &game.id, &bo).await;
    assert_eq!(socket.welcome(&game.id).await.index(), 1);

    sqlx::query("ALTER TABLE seats_away RENAME TO seats")
        .execute(&pool)
        .await
        .unwrap();
    let claim = join(server.addr, &game.id, &json!({ "name": "Cy" }), &[]).await;
    assert_eq!(claim.status, 200, "{}", claim.text());
    assert_eq!(claim.json()["seat"], 2);
}

#[sqlx::test(migrations = false)]
async fn the_code_of_a_game_made_by_an_account_joins_like_any_other(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let game = make_as(&server, &ann, 2, "invite").await;
    let reply = join_code(server.addr, &game.code, Some("Guest")).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    // The record holds the guest's name and token digest and no account.
    let rows: Vec<(i32, String, Option<String>)> = sqlx::query_as(
        "SELECT seat, name, user_id::text FROM seats WHERE game_id = $1 ORDER BY seat",
    )
    .bind(uuid::Uuid::parse_str(&game.id).unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[1].0, rows[1].1.as_str(), rows[1].2.as_deref()),
        (1, "Guest", None)
    );
    assert_eq!(rows[0].2.as_deref(), Some(ann.id.as_str()));
}
