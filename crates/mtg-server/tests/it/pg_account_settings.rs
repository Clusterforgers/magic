//! Goal: the account settings work end to end over real HTTP on Postgres: a password changes only for the signed-in owner who knows the current one and chooses a new one within the rules, the change ends every other session and keeps the one that made it, wrong confirmations are throttled with the sign-in counters, and a deleted account takes its decks, sessions and seat bindings in one step, gives back the seats a join took, closes the lobby it hosted, keeps its seat in a running game as a guest seat with the connection closed, leaves no row that names it, frees its username and hands nothing to a later account of the same name.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;

use std::time::Duration;

use mtg_db::{Database, DeckDraft};
use mtg_server::config::AttemptLimits;
use mtg_server::{ServerConfig, migrate};
use net::lobby::{self, Made, join};
use net::{
    Browser, PASSWORD, Reply, TestServer, bot_config, credentials, login, me, register, request,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

const NEW_PASSWORD: &str = "another long passphrase";

fn roomy_limits() -> AttemptLimits {
    AttemptLimits {
        window: Duration::from_secs(600),
        login_per_ip: 10_000,
        login_per_username: 10_000,
        register_per_ip: 10_000,
        code_window: Duration::from_secs(600),
        code_misses_per_ip: 10_000,
        lobby_window: Duration::from_secs(600),
        lobby_requests_per_ip: 10_000,
    }
}

/// Both schemas migrated behind a running server, after `change` has adjusted the configuration.
async fn start_with(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> TestServer {
    migrate(pool).await.unwrap();
    Database::from_pool(pool.clone()).migrate().await.unwrap();
    let mut config = bot_config();
    config.auth.limits = roomy_limits();
    change(&mut config);
    TestServer::start_on(pool.clone(), config).await
}

async fn start(pool: &PgPool) -> TestServer {
    start_with(pool, |_| {}).await
}

fn change_body(current: &str, new: &str) -> String {
    json!({ "current_password": current, "new_password": new }).to_string()
}

/// `POST /api/auth/password` as `who`.
async fn change(server: &TestServer, who: &Browser, current: &str, new: &str) -> Reply {
    who.post(
        server.addr,
        "/api/auth/password",
        &change_body(current, new),
    )
    .await
}

/// `DELETE /api/auth/account` as `who`, confirmed with `password`.
async fn delete(server: &TestServer, who: &Browser, password: &str) -> Reply {
    let body = json!({ "password": password }).to_string();
    who.send(server.addr, "DELETE", "/api/auth/account", &body, &[])
        .await
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn hash_of(pool: &PgPool, who: &Browser) -> String {
    sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
        .bind(Uuid::parse_str(&who.id).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

fn sets_no_cookie(reply: &Reply) {
    let lines = reply.headers_named("set-cookie");
    assert_eq!(lines.len(), 0, "unexpected cookies: {lines:?}");
}

async fn signs_in(server: &TestServer, username: &str, password: &str) -> Reply {
    request(
        server.addr,
        "POST",
        "/api/auth/login",
        &[("Content-Type", "application/json")],
        credentials(username, password).as_bytes(),
    )
    .await
}

// --- the password ---------------------------------------------------------------------------

#[sqlx::test(migrations = false)]
async fn changing_the_password_ends_the_other_sessions_and_keeps_this_one(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let laptop = register(addr, "Ann", PASSWORD).await;
    let phone = login(addr, "Ann", PASSWORD).await;
    let stolen = login(addr, "ann", PASSWORD).await;
    let bob = register(addr, "Bob", PASSWORD).await;
    assert_eq!(count(&pool, "SELECT count(*) FROM sessions").await, 4);
    let before = hash_of(&pool, &phone).await;
    let bobs = hash_of(&pool, &bob).await;

    let reply = change(&server, &phone, PASSWORD, NEW_PASSWORD).await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    assert_eq!(reply.body.len(), 0);
    sets_no_cookie(&reply);

    // The session that changed it stays; the account's others end; another account's stays.
    assert_eq!(phone.get(addr, "/api/auth/me").await.status, 200);
    for gone in [&laptop, &stolen] {
        let reply = gone.get(addr, "/api/auth/me").await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (401, "unauthenticated")
        );
    }
    assert_eq!(bob.get(addr, "/api/auth/me").await.status, 200);
    assert_eq!(count(&pool, "SELECT count(*) FROM sessions").await, 2);

    // The old password is dead and the new one signs in.
    let old = signs_in(&server, "Ann", PASSWORD).await;
    assert_eq!(
        (old.status, old.error_code().as_str()),
        (401, "invalid_credentials")
    );
    login(addr, "Ann", NEW_PASSWORD).await;

    // Only a fresh Argon2id hash is stored, never the password; nobody else's hash moved.
    let after = hash_of(&pool, &phone).await;
    assert_ne!(after, before);
    assert!(after.starts_with("$argon2id$"), "{after}");
    assert!(!after.contains(NEW_PASSWORD));
    assert_eq!(hash_of(&pool, &bob).await, bobs);
}

#[sqlx::test(migrations = false)]
async fn a_wrong_current_password_changes_nothing(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let other = login(addr, "Ann", PASSWORD).await;
    let before = hash_of(&pool, &ann).await;

    // A wrong one, the new one instead of the current one, and three that break the rules and so
    // cannot be anybody's password.
    for current in [
        "not the password at all",
        NEW_PASSWORD,
        "",
        "short",
        &"a".repeat(129),
    ] {
        let reply = change(&server, &ann, current, NEW_PASSWORD).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (401, "invalid_credentials"),
            "{current:?}"
        );
        assert_eq!(reply.json()["message"], "wrong password");
        sets_no_cookie(&reply);
    }
    assert_eq!(hash_of(&pool, &ann).await, before);
    assert_eq!(count(&pool, "SELECT count(*) FROM sessions").await, 2);
    assert_eq!(other.get(addr, "/api/auth/me").await.status, 200);
    login(addr, "Ann", PASSWORD).await;
}

#[sqlx::test(migrations = false)]
async fn a_new_password_outside_the_rules_is_refused_without_echoing_it(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let before = hash_of(&pool, &ann).await;

    for new in ["", "123456789", "éééééééee", &"a".repeat(129)] {
        let reply = change(&server, &ann, PASSWORD, new).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (400, "invalid_password"),
            "{new:?}"
        );
        let message = reply.json()["message"].as_str().unwrap().to_owned();
        assert!(
            message.contains("10") && message.contains("128"),
            "{message}"
        );
        assert!(
            new.is_empty() || !reply.text().contains(new),
            "{}",
            reply.text()
        );
    }
    assert_eq!(hash_of(&pool, &ann).await, before);
}

#[sqlx::test(migrations = false)]
async fn passwords_are_counted_in_characters_at_both_limits(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let mut current = PASSWORD.to_owned();
    // Ten characters of two bytes each, then the longest allowed, then a space-padded one.
    for next in [
        "éééééééééé".to_owned(),
        "x".repeat(128),
        format!("  {PASSWORD}  "),
    ] {
        let reply = change(&server, &ann, &current, &next).await;
        assert_eq!(reply.status, 204, "{}: {}", next.len(), reply.text());
        current = next;
    }
    // Compared byte for byte: the padding matters.
    let trimmed = signs_in(&server, "Ann", PASSWORD).await;
    assert_eq!(trimmed.status, 401);
    login(server.addr, "Ann", &current).await;
}

#[sqlx::test(migrations = false)]
async fn only_a_signed_in_owner_from_this_site_with_a_proper_body_reaches_the_password(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let before = hash_of(&pool, &ann).await;
    let body = change_body(PASSWORD, NEW_PASSWORD);
    let json = [("Content-Type", "application/json")];

    // No session: told to sign in, before the body is read, and no cookie to clear.
    for sent in [body.as_str(), "not json at all"] {
        let reply = request(addr, "POST", "/api/auth/password", &json, sent.as_bytes()).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (401, "unauthenticated")
        );
        sets_no_cookie(&reply);
    }
    // A cookie the server does not honour is told to go.
    let stale = format!("mtg_session={}", "ab".repeat(32));
    let reply = request(
        addr,
        "POST",
        "/api/auth/password",
        &[json[0], ("Cookie", &stale)],
        body.as_bytes(),
    )
    .await;
    assert_eq!(reply.status, 401);
    assert!(reply.headers_named("set-cookie")[0].contains("Max-Age=0"));

    // Another site cannot act with the visitor's cookie.
    for foreign in ["https://evil.example", "null", "http://127.0.0.1:9"] {
        let reply = ann
            .send(
                addr,
                "POST",
                "/api/auth/password",
                &body,
                &[("Origin", foreign)],
            )
            .await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (403, "forbidden_origin"),
            "{foreign}"
        );
        let reply = ann
            .send(
                addr,
                "DELETE",
                "/api/auth/account",
                &json!({"password": PASSWORD}).to_string(),
                &[("Origin", foreign)],
            )
            .await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (403, "forbidden_origin"),
            "{foreign}"
        );
    }

    // Badly formed bodies.
    for (sent, headers, status, code) in [
        ("{}", &json[..], 400, "invalid_request"),
        (
            r#"{"new_password":"abcdefghijkl"}"#,
            &json[..],
            400,
            "invalid_request",
        ),
        (
            r#"{"current_password":"abcdefghijkl","new_password":"abcdefghijkl","x":1}"#,
            &json[..],
            400,
            "invalid_request",
        ),
        (
            r#"{"password":"abcdefghijkl"}"#,
            &json[..],
            400,
            "invalid_request",
        ),
        (body.as_str(), &[][..], 415, "unsupported_media_type"),
    ] {
        let mut all = headers.to_vec();
        all.push(("Cookie", ann.cookie.as_str()));
        let reply = request(addr, "POST", "/api/auth/password", &all, sent.as_bytes()).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (status, code),
            "{sent}"
        );
    }
    let big = json!({"current_password": "a".repeat(40_000), "new_password": NEW_PASSWORD});
    let reply = ann.post(addr, "/api/auth/password", &big.to_string()).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (413, "payload_too_large")
    );

    // The verbs the routes do not take.
    let reply = ann.get(addr, "/api/auth/password").await;
    assert_eq!(reply.status, 405);
    let reply = ann.post(addr, "/api/auth/account", &body).await;
    assert_eq!(reply.status, 405);

    assert_eq!(hash_of(&pool, &ann).await, before);
    assert_eq!(count(&pool, "SELECT count(*) FROM users").await, 1);
}

#[tokio::test]
async fn without_a_database_both_endpoints_say_there_are_no_accounts() {
    let server = TestServer::start(bot_config()).await;
    let json = [("Content-Type", "application/json")];
    for (method, path, body) in [
        (
            "POST",
            "/api/auth/password",
            change_body(PASSWORD, NEW_PASSWORD),
        ),
        (
            "DELETE",
            "/api/auth/account",
            json!({"password": PASSWORD}).to_string(),
        ),
    ] {
        let reply = request(server.addr, method, path, &json, body.as_bytes()).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (503, "accounts_unavailable"),
            "{path}"
        );
    }
}

// --- throttling -----------------------------------------------------------------------------

#[sqlx::test(migrations = false)]
async fn wrong_confirmations_are_counted_with_the_sign_in_tries_and_block_even_the_right_password(
    pool: PgPool,
) {
    let server = start_with(&pool, |config| {
        config.auth.limits.login_per_username = 3;
    })
    .await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let bob = register(addr, "Bob", PASSWORD).await;
    let before = hash_of(&pool, &ann).await;

    // Two wrong passwords at the password form and one at the deletion form use up the budget.
    for _ in 0..2 {
        assert_eq!(
            change(&server, &ann, "wrong wrong wrong", NEW_PASSWORD)
                .await
                .status,
            401
        );
    }
    assert_eq!(delete(&server, &ann, "wrong wrong wrong").await.status, 401);

    // From here even the right password is turned away, at both forms and at the sign-in form.
    let blocked = change(&server, &ann, PASSWORD, NEW_PASSWORD).await;
    assert_eq!(
        (blocked.status, blocked.error_code().as_str()),
        (429, "too_many_attempts")
    );
    let wait: u64 = blocked.header("retry-after").unwrap().parse().unwrap();
    assert!((1..=600).contains(&wait), "{wait}");
    assert_eq!(delete(&server, &ann, PASSWORD).await.status, 429);
    assert_eq!(signs_in(&server, "Ann", PASSWORD).await.status, 429);

    // Nothing happened to the account, and another account is not held up.
    assert_eq!(hash_of(&pool, &ann).await, before);
    assert_eq!(count(&pool, "SELECT count(*) FROM users").await, 2);
    assert_eq!(
        change(&server, &bob, PASSWORD, NEW_PASSWORD).await.status,
        204
    );
}

#[sqlx::test(migrations = false)]
async fn a_right_password_gives_its_try_back(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.limits.login_per_username = 2;
    })
    .await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let mut current = PASSWORD.to_owned();
    // Six requests against a budget of two: each success clears what the failure before it cost.
    for round in 0..3 {
        assert_eq!(
            change(&server, &ann, "wrong wrong wrong", NEW_PASSWORD)
                .await
                .status,
            401
        );
        let next = format!("{NEW_PASSWORD} {round}");
        let reply = change(&server, &ann, &current, &next).await;
        assert_eq!(reply.status, 204, "round {round}: {}", reply.text());
        current = next;
    }
}

// --- deleting the account -------------------------------------------------------------------

/// Two accounts with decks and sessions, and the games the deletion has to deal with.
struct World {
    server: TestServer,
    /// Ann's session and a second one.
    ann: Browser,
    ann_phone: Browser,
    bob: Browser,
    /// Bob's public lobby, which Ann joined with the token she was given.
    bobs_lobby: Made,
    ann_seat_token: String,
    /// Ann's public lobby with an open seat.
    anns_lobby: Made,
    /// A game Ann hosts, Bob joined and that is running.
    running: Made,
}

async fn world(pool: &PgPool) -> World {
    let server = start(pool).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let ann_phone = login(addr, "Ann", PASSWORD).await;
    let bob = register(addr, "Bob", PASSWORD).await;

    let database = Database::from_pool(pool.clone());
    for (owner, names) in [(&ann, &["One", "Two", "Three"][..]), (&bob, &["Bob's"][..])] {
        for name in names {
            let draft = DeckDraft {
                name: (*name).to_owned(),
                format: "commander".to_owned(),
                notes: String::new(),
                lines: vec![],
                check: None,
            };
            let owner = Uuid::parse_str(&owner.id).unwrap();
            database.decks().create(owner, &draft, 100).await.unwrap();
        }
    }

    let bobs_lobby = make_as(&server, &bob, 2).await;
    let joined = join(addr, &bobs_lobby.id, &json!({}), &[("Cookie", &ann.cookie)]).await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    let ann_seat_token = joined.json()["token"].as_str().unwrap().to_owned();

    let anns_lobby = make_as(&server, &ann, 3).await;

    let running = make_as(&server, &ann, 2).await;
    let joined = join(addr, &running.id, &json!({}), &[("Cookie", &bob.cookie)]).await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    lobby::start(&server.registry, &running.id, 2).await;

    World {
        server,
        ann,
        ann_phone,
        bob,
        bobs_lobby,
        ann_seat_token,
        anns_lobby,
        running,
    }
}

/// A public game of `seats` seats made by `host`, whose seat 0 is held and bound.
async fn make_as(server: &TestServer, host: &Browser, seats: usize) -> Made {
    let body = lobby::body(&host.username, seats, 1, "public").to_string();
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

/// Every column of the public schema that can hold an id or a name as text, with the number of
/// rows whose value contains `needle`, for the ones that have any.
async fn mentions(pool: &PgPool, needle: &str) -> Vec<String> {
    let columns: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name, column_name FROM information_schema.columns
         WHERE table_schema = 'public'
           AND data_type IN ('uuid', 'text', 'character varying', 'json', 'jsonb')
         ORDER BY table_name, column_name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut found = Vec::new();
    for (table, column) in columns {
        let sql =
            format!("SELECT count(*) FROM \"{table}\" WHERE \"{column}\"::text LIKE '%{needle}%'");
        let rows = count(pool, &sql).await;
        if rows > 0 {
            found.push(format!("{table}.{column}: {rows}"));
        }
    }
    found
}

async fn seat_row(pool: &PgPool, game: &str, seat: i32) -> (String, String, Option<Uuid>, bool) {
    sqlx::query_as(
        "SELECT state, name, user_id, token_digest IS NOT NULL FROM seats
         WHERE game_id = $1 AND seat = $2",
    )
    .bind(Uuid::parse_str(game).unwrap())
    .bind(seat)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn status_of(pool: &PgPool, game: &str) -> String {
    sqlx::query_scalar("SELECT status FROM games WHERE id = $1")
        .bind(Uuid::parse_str(game).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_removes_every_row_that_names_it_and_only_those(pool: PgPool) {
    let w = world(&pool).await;
    let addr = w.server.addr;
    let (ann_id, bob_id) = (w.ann.id.clone(), w.bob.id.clone());
    let others_before = mentions(&pool, &bob_id).await;

    // The scan finds the account in each table that refers to it.
    let before = mentions(&pool, &ann_id).await;
    for table in [
        "decks.owner",
        "seats.user_id",
        "sessions.user_id",
        "users.id",
    ] {
        assert!(
            before.iter().any(|line| line.starts_with(table)),
            "{table} missing from {before:?}"
        );
    }
    assert_eq!(count(&pool, "SELECT count(*) FROM decks").await, 4);

    let reply = delete(&w.server, &w.ann, PASSWORD).await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    assert_eq!(reply.body.len(), 0);
    let cleared = reply.headers_named("set-cookie");
    assert!(
        cleared.len() == 1
            && cleared[0].starts_with("mtg_session=;")
            && cleared[0].contains("Max-Age=0"),
        "{cleared:?}"
    );

    assert_eq!(mentions(&pool, &ann_id).await, Vec::<String>::new());
    assert_eq!(count(&pool, "SELECT count(*) FROM users").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM sessions").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM decks").await, 1);
    // Nothing of the other account changed.
    assert_eq!(mentions(&pool, &bob_id).await, others_before);
    assert_eq!(w.bob.get(addr, "/api/auth/me").await.status, 200);

    // Both of Ann's sessions are dead, and she cannot sign in.
    for session in [&w.ann, &w.ann_phone] {
        let reply = session.get(addr, "/api/auth/me").await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (401, "unauthenticated")
        );
        assert_eq!(session.get(addr, "/api/me/games").await.status, 401);
    }
    let reply = signs_in(&w.server, "Ann", PASSWORD).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (401, "invalid_credentials")
    );
    // A second deletion finds no session.
    assert_eq!(delete(&w.server, &w.ann_phone, PASSWORD).await.status, 401);
}

#[sqlx::test(migrations = false)]
async fn a_wrong_password_deletes_nothing(pool: PgPool) {
    let w = world(&pool).await;
    let before = mentions(&pool, &w.ann.id).await;
    for password in [
        "wrong wrong wrong",
        "",
        "short",
        &"a".repeat(129),
        NEW_PASSWORD,
    ] {
        let reply = delete(&w.server, &w.ann, password).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (401, "invalid_credentials"),
            "{password:?}"
        );
        sets_no_cookie(&reply);
    }
    assert_eq!(mentions(&pool, &w.ann.id).await, before);
    assert_eq!(w.server.registry.len(), 3);
    assert_eq!(w.ann.get(w.server.addr, "/api/auth/me").await.status, 200);
}

#[sqlx::test(migrations = false)]
async fn a_seat_a_join_took_is_given_back_and_the_lobby_the_account_hosts_is_closed(pool: PgPool) {
    let w = world(&pool).await;
    let addr = w.server.addr;
    let bobs = &w.bobs_lobby.id;
    assert_eq!(seat_row(&pool, bobs, 1).await.0, "claimed");
    let leave = format!("/api/games/{bobs}/leave");
    let token = format!("Bearer {}", w.ann_seat_token);
    let host_socket_path = format!("/ws/{}", w.anns_lobby.id);
    let mut socket = net::open(addr, &host_socket_path, &[("Cookie", &w.ann.cookie)]).await;
    assert_eq!(socket.welcome(&w.anns_lobby.id).await.index(), 0);
    let mut seat_socket =
        net::open(addr, &format!("/ws/{bobs}"), &[("Cookie", &w.ann.cookie)]).await;
    assert_eq!(seat_socket.welcome(bobs).await.index(), 1);

    assert_eq!(delete(&w.server, &w.ann, PASSWORD).await.status, 204);

    // Bob's lobby has its open seat again: no name, no token, no account, and Ann's token is dead.
    assert_eq!(
        seat_row(&pool, bobs, 1).await,
        ("open".into(), String::new(), None, false)
    );
    let reply = lobby::call(addr, "POST", &leave, None, &[("Authorization", &token)]).await;
    assert_eq!(reply.status, 404);
    let listed = lobby::listed(&lobby::lobbies(addr, &[]).await);
    assert!(listed.contains(bobs), "{listed:?}");
    let info = lobby::call(addr, "GET", &format!("/api/games/{bobs}"), None, &[]).await;
    let seats = info.json()["seats"].clone();
    let names: Vec<&str> = seats
        .as_array()
        .unwrap()
        .iter()
        .map(|seat| seat["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Bob", ""]);
    assert_eq!(seat_socket.closed().await, Some(4001));

    // Ann's own lobby is closed: gone from the registry, the listing, the lookup and the code.
    let hers = &w.anns_lobby.id;
    assert_eq!(status_of(&pool, hers).await, "expired");
    assert_eq!(socket.closed().await, Some(4002));
    assert!(!listed.contains(hers), "{listed:?}");
    let info = lobby::call(addr, "GET", &format!("/api/games/{hers}"), None, &[]).await;
    assert_eq!(info.status, 404);
    let by_code = lobby::join_code(addr, &w.anns_lobby.code, Some("Late")).await;
    assert_eq!(by_code.status, 404);
    assert_eq!(w.server.registry.len(), 2);
}

#[sqlx::test(migrations = false)]
async fn the_host_of_a_running_game_keeps_the_seat_as_a_guest_seat(pool: PgPool) {
    running_game_keeps_the_seat(&pool, true).await;
}

#[sqlx::test(migrations = false)]
async fn a_joiner_of_a_running_game_keeps_the_seat_as_a_guest_seat(pool: PgPool) {
    running_game_keeps_the_seat(&pool, false).await;
}

/// Deletes the host (seat 0, made with the game) or the joiner (seat 1, claimed) of the running
/// game: the game goes on at its table, the seat keeps the name it was played under and has no
/// account, the deleted player's connection closes and the other player is undisturbed.
async fn running_game_keeps_the_seat(pool: &PgPool, host_deleted: bool) {
    let w = world(pool).await;
    let addr = w.server.addr;
    let game = &w.running.id;
    let (gone, kept, seat, other) = if host_deleted {
        (&w.ann, &w.bob, 0_usize, 1_usize)
    } else {
        (&w.bob, &w.ann, 1, 0)
    };
    let number = |seat: usize| i32::try_from(seat).unwrap();
    let path = format!("/ws/{game}");
    let mut leaving = net::open(addr, &path, &[("Cookie", &gone.cookie)]).await;
    assert_eq!(leaving.welcome(game).await.index(), seat);
    let mut staying = net::open(addr, &path, &[("Cookie", &kept.cookie)]).await;
    assert_eq!(staying.welcome(game).await.index(), other);
    let name = gone.username.clone();

    assert_eq!(delete(&w.server, gone, PASSWORD).await.status, 204);

    // The game goes on at its table; the seat keeps the name it was played under and has no
    // account. The deleted player's own lobby went with them.
    assert_eq!(status_of(pool, game).await, "running");
    assert_eq!(w.server.registry.len(), 2);
    let (state, shown, user, has_token) = seat_row(pool, game, number(seat)).await;
    assert_eq!((shown.as_str(), user), (name.as_str(), None));
    assert!(has_token && state != "open", "{state}");
    leaving.closed().await;
    let holder = seat_row(pool, game, number(other)).await.2;
    assert_eq!(holder, Some(Uuid::parse_str(&kept.id).unwrap()));

    // The other player's list still has the game and shows the seat without an account.
    let listed = kept.games(addr).await;
    let mine = listed
        .iter()
        .find(|entry| entry["game"] == *game)
        .expect("the game is listed");
    let entry = mine["others"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["seat"] == seat)
        .expect("the deleted player's seat");
    assert_eq!(entry["name"], name);
    assert_eq!(entry["user"], Value::Null);
    assert_eq!(mine["status"], "running");
    drop(staying);

    // The deleted session opens nothing.
    let (status, _) = net::refused_at(addr, &path, &[("Cookie", &gone.cookie)]).await;
    assert_eq!(status, 401);
}

#[sqlx::test(migrations = false)]
async fn the_username_is_free_again_and_a_new_account_of_that_name_inherits_nothing(pool: PgPool) {
    let w = world(&pool).await;
    let addr = w.server.addr;
    let old_id = w.ann.id.clone();
    assert_eq!(delete(&w.server, &w.ann, PASSWORD).await.status, 204);

    let again = register(addr, "Ann", NEW_PASSWORD).await;
    assert_ne!(again.id, old_id);
    assert_eq!(again.games(addr).await, Vec::<Value>::new());
    let decks = again.get(addr, "/api/decks").await;
    assert_eq!(decks.status, 200, "{}", decks.text());
    assert_eq!(decks.json()["decks"], json!([]));
    let socket = format!("/ws/{}", w.running.id);
    let (status, _) = net::refused_at(addr, &socket, &[("Cookie", &again.cookie)]).await;
    assert_eq!(status, 404);
    assert_eq!(seat_row(&pool, &w.running.id, 0).await.2, None);
    assert_eq!(me(addr, Some(&again.cookie)).await.status, 200);
}

#[sqlx::test(migrations = false)]
async fn racing_deletions_have_one_winner(pool: PgPool) {
    let w = world(&pool).await;
    let (a, b, c) = tokio::join!(
        delete(&w.server, &w.ann, PASSWORD),
        delete(&w.server, &w.ann, PASSWORD),
        delete(&w.server, &w.ann_phone, PASSWORD),
    );
    let mut statuses = [a.status, b.status, c.status];
    statuses.sort_unstable();
    assert_eq!(
        statuses,
        [204, 401, 401],
        "{}{}{}",
        a.text(),
        b.text(),
        c.text()
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM users").await, 1);
    assert_eq!(mentions(&pool, &w.ann.id).await, Vec::<String>::new());
}

// --- logs -----------------------------------------------------------------------------------

#[sqlx::test(migrations = false)]
async fn neither_password_reaches_the_logs(pool: PgPool) {
    let logs = common::capture_logs();
    let server = start(&pool).await;
    let username = format!("Set_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let old = format!("old-{}", Uuid::new_v4());
    let new = format!("new-{}", Uuid::new_v4());
    let wrong = format!("wrong-{}", Uuid::new_v4());
    let ann = register(server.addr, &username, &old).await;

    assert_eq!(change(&server, &ann, &wrong, &new).await.status, 401);
    assert_eq!(change(&server, &ann, &old, &new).await.status, 204);
    assert_eq!(delete(&server, &ann, &wrong).await.status, 401);
    assert_eq!(delete(&server, &ann, &new).await.status, 204);

    let text = String::from_utf8_lossy(&logs.lock().unwrap()).into_owned();
    assert!(text.contains("password changed"), "the capture works");
    assert!(text.contains("account deleted"), "the capture works");
    for (what, secret) in [
        ("the old password", old.as_str()),
        ("the new password", new.as_str()),
        ("a wrong password", wrong.as_str()),
        ("the session token", ann.token.as_str()),
        ("the username", username.as_str()),
    ] {
        assert!(!text.contains(secret), "the logs contain {what}");
    }
}
