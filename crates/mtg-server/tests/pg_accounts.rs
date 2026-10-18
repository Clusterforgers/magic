//! Goal: accounts work end to end over real HTTP on Postgres: sign up, sign in, who-am-I and sign out with a session cookie that has exactly the documented attributes; usernames and passwords are validated at their limits and unique ignoring case; only hashes and digests are stored; a failed login says nothing about whether the username exists; guessing is throttled per username and per address; registration can be closed; sessions end by expiry, logout or replacement; foreign sites change nothing; a server without a database says so; and neither passwords nor tokens reach the logs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::io;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use mtg_server::config::{AttemptLimits, Registration};
use mtg_server::{ServerConfig, UserId, migrate};
use net::{
    Browser, PASSWORD, Reply, TestServer, bot_config, credentials, login, me, register, request,
    session_cookie,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// Limits no test trips by accident; the throttle tests set their own.
fn roomy_limits() -> AttemptLimits {
    AttemptLimits {
        window: Duration::from_secs(600),
        login_per_ip: 10_000,
        login_per_username: 10_000,
        register_per_ip: 10_000,
    }
}

/// A migrated database behind a running server, after `change` has adjusted the configuration.
async fn start_with(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> TestServer {
    migrate(pool).await.unwrap();
    let mut config = bot_config();
    config.auth.limits = roomy_limits();
    change(&mut config);
    TestServer::start_on(pool.clone(), config).await
}

async fn start(pool: &PgPool) -> TestServer {
    start_with(pool, |_| {}).await
}

async fn post(server: &TestServer, path: &str, body: &str, extra: &[(&str, &str)]) -> Reply {
    let mut headers = vec![("Content-Type", "application/json")];
    headers.extend_from_slice(extra);
    request(server.addr, "POST", path, &headers, body.as_bytes()).await
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    // `table` is always one of this file's own literals.
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Everything in a reply that does not change from one identical failure to the next.
fn fingerprint(reply: &Reply) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let headers = reply
        .headers
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("date"))
        .cloned()
        .collect();
    (reply.status, headers, reply.body.clone())
}

/// One request of the table in the bad-bodies test: what it is, its headers, body, and the
/// status and error code it must get.
type Case<'a> = (&'a str, Vec<(&'a str, &'a str)>, String, u16, &'a str);

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, byte| {
        write!(out, "{byte:02x}").unwrap();
        out
    })
}

/// The reply sets no cookie at all.
fn sets_no_cookie(reply: &Reply) {
    let lines = reply.headers_named("set-cookie");
    assert_eq!(lines.len(), 0, "unexpected cookies: {lines:?}");
}

fn attributes(set_cookie: &str) -> Vec<&str> {
    set_cookie.split("; ").collect()
}

#[sqlx::test(migrations = false)]
async fn registering_signs_in_and_logging_out_ends_the_session(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;

    let ann = register(addr, "Ann_1", PASSWORD).await;
    assert_eq!(ann.username, "Ann_1");
    assert!(ann.id.parse::<UserId>().is_ok(), "{}", ann.id);
    assert_eq!(
        (count(&pool, "users").await, count(&pool, "sessions").await),
        (1, 1)
    );

    let who = ann.get(addr, "/api/auth/me").await;
    assert_eq!(who.status, 200);
    assert_eq!(who.json(), json!({"id": ann.id, "username": "Ann_1"}));
    assert_eq!(who.header("cache-control"), Some("no-store"));
    sets_no_cookie(&who);

    let out = ann.post(addr, "/api/auth/logout", "").await;
    assert_eq!(out.status, 204);
    assert_eq!(out.body.len(), 0);
    let cleared = out.headers_named("set-cookie");
    assert_eq!(cleared.len(), 1);
    assert!(cleared[0].starts_with("mtg_session=;"), "{cleared:?}");
    assert!(cleared[0].contains("Max-Age=0"), "{cleared:?}");
    assert_eq!(count(&pool, "sessions").await, 0);

    // The old cookie is dead, and a request that presents it is told to drop it.
    let stale = ann.get(addr, "/api/auth/me").await;
    assert_eq!(
        (stale.status, stale.error_code().as_str()),
        (401, "unauthenticated")
    );
    let told = stale.headers_named("set-cookie");
    assert!(told.len() == 1 && told[0].contains("Max-Age=0"), "{told:?}");
    // No cookie at all is the same 401 without the instruction.
    let anonymous = me(addr, None).await;
    assert_eq!(
        (anonymous.status, anonymous.error_code().as_str()),
        (401, "unauthenticated")
    );
    sets_no_cookie(&anonymous);

    // Logging out twice, or with no session, is fine.
    assert_eq!(ann.post(addr, "/api/auth/logout", "").await.status, 204);
    assert_eq!(post(&server, "/api/auth/logout", "", &[]).await.status, 204);

    // Signing in again starts a new session; the name comes back as it was registered.
    let again = login(addr, "ann_1", PASSWORD).await;
    assert_ne!(again.token, ann.token);
    assert_eq!(
        (again.id.as_str(), again.username.as_str()),
        (ann.id.as_str(), "Ann_1")
    );
    assert_eq!(me(addr, Some(&again.cookie)).await.status, 200);
}

#[sqlx::test(migrations = false)]
async fn two_browsers_hold_two_sessions_and_each_logs_out_alone(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let phone = register(addr, "Ann", PASSWORD).await;
    let laptop = login(addr, "Ann", PASSWORD).await;
    assert_eq!(count(&pool, "sessions").await, 2);

    assert_eq!(phone.post(addr, "/api/auth/logout", "").await.status, 204);
    assert_eq!(me(addr, Some(&phone.cookie)).await.status, 401);
    assert_eq!(me(addr, Some(&laptop.cookie)).await.status, 200);
    assert_eq!(count(&pool, "sessions").await, 1);
}

#[sqlx::test(migrations = false)]
async fn the_session_cookie_has_exactly_the_documented_attributes(pool: PgPool) {
    for (round, secure) in [false, true].into_iter().enumerate() {
        let server = start_with(&pool, |config| config.auth.cookie_secure = secure).await;
        let name = format!("cookie_{round}");
        let body = credentials(&name, PASSWORD);
        let registered = post(&server, "/api/auth/register", &body, &[]).await;
        let logged_in = post(&server, "/api/auth/login", &body, &[]).await;
        let (registered, logged_in) = (&registered, &logged_in);
        assert_eq!((registered.status, logged_in.status), (201, 200));

        for reply in [registered, logged_in] {
            let lines = reply.headers_named("set-cookie");
            assert_eq!(lines.len(), 1, "{lines:?}");
            let parts = attributes(lines[0]);
            let (pair, rest) = parts.split_first().unwrap();
            let token = pair.strip_prefix("mtg_session=").unwrap();
            assert_eq!(token.len(), 64, "256 bits");
            assert!(
                token
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            );
            let mut expected = vec!["Max-Age=2592000", "Path=/", "HttpOnly", "SameSite=Lax"];
            if secure {
                expected.push("Secure");
            }
            assert_eq!(rest, expected, "secure = {secure}");
            assert!(!lines[0].contains("Domain"));
        }
    }
    // Thirty days by the database's clock, whatever the cookie says.
    let spans: Vec<f64> = sqlx::query_scalar(
        "SELECT extract(epoch FROM expires_at - created_at)::float8 FROM sessions",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(spans.len(), 4);
    assert!(
        spans.iter().all(|span| (span - 2_592_000.0).abs() < 5.0),
        "{spans:?}"
    );
}

#[sqlx::test(migrations = false)]
async fn the_lifetime_of_a_session_follows_the_configuration(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.session_lifetime = Duration::from_secs(90);
    })
    .await;
    let reply = post(
        &server,
        "/api/auth/register",
        &credentials("brief", PASSWORD),
        &[],
    )
    .await;
    assert_eq!(reply.status, 201);
    assert!(reply.headers_named("set-cookie")[0].contains("Max-Age=90;"));
    let span: f64 = sqlx::query_scalar(
        "SELECT extract(epoch FROM expires_at - created_at)::float8 FROM sessions",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!((span - 90.0).abs() < 5.0, "{span}");
}

#[sqlx::test(migrations = false)]
async fn usernames_and_passwords_are_checked_at_their_limits(pool: PgPool) {
    let server = start(&pool).await;
    let longest_name = "n".repeat(32);
    let longest_password = "p".repeat(128);
    let cases: Vec<(String, String, u16, &str)> = vec![
        ("abc".into(), PASSWORD.into(), 201, ""),
        (longest_name.clone(), longest_password.clone(), 201, ""),
        ("a-b_c".into(), "p".repeat(10), 201, ""),
        ("unicode_pw".into(), "é".repeat(10), 201, ""),
        ("spaces_pw".into(), " ".repeat(10), 201, ""),
        ("ab".into(), PASSWORD.into(), 400, "invalid_username"),
        (String::new(), PASSWORD.into(), 400, "invalid_username"),
        ("n".repeat(33), PASSWORD.into(), 400, "invalid_username"),
        ("has space".into(), PASSWORD.into(), 400, "invalid_username"),
        (" leading".into(), PASSWORD.into(), 400, "invalid_username"),
        ("trailing ".into(), PASSWORD.into(), 400, "invalid_username"),
        ("tab\there".into(), PASSWORD.into(), 400, "invalid_username"),
        ("new\nline".into(), PASSWORD.into(), 400, "invalid_username"),
        ("nul\0byte".into(), PASSWORD.into(), 400, "invalid_username"),
        ("dot.name".into(), PASSWORD.into(), 400, "invalid_username"),
        ("zoë".into(), PASSWORD.into(), 400, "invalid_username"),
        ("аnna".into(), PASSWORD.into(), 400, "invalid_username"), // Cyrillic а
        ("emoji🙂x".into(), PASSWORD.into(), 400, "invalid_username"),
        (
            "a\u{202e}bc".into(),
            PASSWORD.into(),
            400,
            "invalid_username",
        ),
        ("fine_name".into(), "p".repeat(9), 400, "invalid_password"),
        ("fine_name".into(), String::new(), 400, "invalid_password"),
        ("fine_name".into(), "é".repeat(9), 400, "invalid_password"),
        ("fine_name".into(), "p".repeat(129), 400, "invalid_password"),
        ("fine_name".into(), "é".repeat(129), 400, "invalid_password"),
        // Both wrong: the username is judged first.
        ("x".into(), "short".into(), 400, "invalid_username"),
    ];
    let mut created = 0;
    for (username, password, status, code) in cases {
        let reply = post(
            &server,
            "/api/auth/register",
            &credentials(&username, &password),
            &[],
        )
        .await;
        assert_eq!(
            reply.status,
            status,
            "{username:?} / {} chars: {}",
            password.chars().count(),
            reply.text()
        );
        if status == 201 {
            created += 1;
            let json = reply.json();
            assert_eq!(json["username"], username.as_str());
            assert!(session_cookie(&reply).is_some());
        } else {
            assert_eq!(reply.error_code(), code, "{username:?}");
            sets_no_cookie(&reply);
            let message = reply.json()["message"].as_str().unwrap().to_owned();
            // The message names the rule, not the input.
            assert!(
                message.contains(if code == "invalid_username" {
                    "32"
                } else {
                    "128"
                }),
                "{message}"
            );
            assert!(
                !message.contains(&password) || password.len() < 5,
                "{message}"
            );
        }
    }
    assert_eq!(created, 5);
    assert_eq!(count(&pool, "users").await, created);

    // What was accepted at the limits works for logging in, too.
    login(server.addr, &longest_name, &longest_password).await;
    login(server.addr, "spaces_pw", &" ".repeat(10)).await;
    login(server.addr, "unicode_pw", &"é".repeat(10)).await;
}

#[sqlx::test(migrations = false)]
async fn usernames_are_unique_ignoring_case_and_shown_as_typed(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    register(addr, "Ann", PASSWORD).await;
    for twin in ["ann", "ANN", "aNn", "Ann"] {
        let reply = post(
            &server,
            "/api/auth/register",
            &credentials(twin, "another long password"),
            &[],
        )
        .await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (409, "username_taken"),
            "{twin}"
        );
        sets_no_cookie(&reply);
    }
    assert_eq!(count(&pool, "users").await, 1);
    assert_eq!(
        count(&pool, "sessions").await,
        1,
        "a refused registration signs nobody in"
    );

    // The first password still opens the account, under any spelling of the name.
    for spelling in ["Ann", "ann", "ANN"] {
        let browser = login(addr, spelling, PASSWORD).await;
        assert_eq!(browser.username, "Ann");
    }
    // The database agrees on its own.
    let clash = sqlx::query("INSERT INTO users (id, username, username_key, password_hash) VALUES ($1, 'ANN', 'ann', 'x')")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await;
    assert!(clash.is_err());
    let bad_key = sqlx::query("INSERT INTO users (id, username, username_key, password_hash) VALUES ($1, 'Bob', 'Bob', 'x')")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await;
    assert!(bad_key.is_err(), "the key must be the lower-case name");
    let bad_name = sqlx::query("INSERT INTO users (id, username, username_key, password_hash) VALUES ($1, 'b o b', 'b o b', 'x')")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await;
    assert!(
        bad_name.is_err(),
        "the database refuses names the server would"
    );
}

#[sqlx::test(migrations = false)]
async fn bodies_that_are_not_credentials_are_refused_before_anything_happens(pool: PgPool) {
    let server = start(&pool).await;
    let big = format!("{}{}", credentials("bigbody", PASSWORD), " ".repeat(20_000));
    let cases: Vec<Case> = vec![
        (
            "text/plain",
            vec![("Content-Type", "text/plain")],
            credentials("plain", PASSWORD),
            415,
            "unsupported_media_type",
        ),
        (
            "no type",
            vec![],
            credentials("plain", PASSWORD),
            415,
            "unsupported_media_type",
        ),
        (
            "form",
            vec![("Content-Type", "application/x-www-form-urlencoded")],
            "username=a&password=b".into(),
            415,
            "unsupported_media_type",
        ),
        (
            "extra field",
            vec![("Content-Type", "application/json")],
            json!({"username": "extra", "password": PASSWORD, "admin": true}).to_string(),
            400,
            "invalid_request",
        ),
        (
            "no password",
            vec![("Content-Type", "application/json")],
            json!({"username": "nopass"}).to_string(),
            400,
            "invalid_request",
        ),
        (
            "no username",
            vec![("Content-Type", "application/json")],
            json!({"password": PASSWORD}).to_string(),
            400,
            "invalid_request",
        ),
        (
            "numbers",
            vec![("Content-Type", "application/json")],
            json!({"username": 5, "password": 6}).to_string(),
            400,
            "invalid_request",
        ),
        (
            "null",
            vec![("Content-Type", "application/json")],
            "null".into(),
            400,
            "invalid_request",
        ),
        (
            "array",
            vec![("Content-Type", "application/json")],
            "[]".into(),
            400,
            "invalid_request",
        ),
        (
            "empty",
            vec![("Content-Type", "application/json")],
            String::new(),
            400,
            "invalid_request",
        ),
        (
            "truncated",
            vec![("Content-Type", "application/json")],
            r#"{"username":"cut""#.into(),
            400,
            "invalid_request",
        ),
        (
            "too large",
            vec![("Content-Type", "application/json")],
            big,
            413,
            "payload_too_large",
        ),
    ];
    for path in ["/api/auth/register", "/api/auth/login"] {
        for (what, headers, body, status, code) in &cases {
            let reply = request(server.addr, "POST", path, headers, body.as_bytes()).await;
            assert_eq!(
                (reply.status, reply.error_code().as_str()),
                (*status, *code),
                "{path}: {what}"
            );
            sets_no_cookie(&reply);
            assert!(reply.body.len() < 400, "{}", reply.text());
        }
    }
    assert_eq!(
        (count(&pool, "users").await, count(&pool, "sessions").await),
        (0, 0)
    );
}

#[sqlx::test(migrations = false)]
async fn only_hashes_and_digests_are_stored_and_nothing_secret_is_echoed(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let registered = {
        let body = credentials("Ann", PASSWORD);
        post(&server, "/api/auth/register", &body, &[]).await
    };
    let first = Browser {
        cookie: session_cookie(&registered).unwrap(),
        token: session_cookie(&registered)
            .unwrap()
            .trim_start_matches("mtg_session=")
            .to_owned(),
        id: registered.json()["id"].as_str().unwrap().to_owned(),
        username: "Ann".into(),
    };
    let second = login(addr, "Ann", PASSWORD).await;

    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$v=19$m=8,t=1,p=1$"), "{hash}");
    assert!(!hash.contains(PASSWORD));

    let digests: Vec<Vec<u8>> = sqlx::query_scalar("SELECT token_digest FROM sessions")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(digests.len(), 2);
    for browser in [&first, &second] {
        let digest: Vec<u8> = Sha256::digest(browser.token.as_bytes()).to_vec();
        assert!(
            digests.contains(&digest),
            "the digest of a session token is stored"
        );
    }

    // No column of either table holds a token or the password in clear.
    for table in ["users", "sessions"] {
        for secret in [PASSWORD, first.token.as_str(), second.token.as_str()] {
            let hits: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {table} t WHERE row_to_json(t)::text LIKE '%' || $1 || '%'"
            )))
            .bind(secret)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(hits, 0, "{table} holds a secret");
        }
    }
    // Bodies never repeat the password or the token, and the token travels only in the header.
    assert!(!registered.text().contains(PASSWORD) && !registered.text().contains(&first.token));
    let who = first.get(addr, "/api/auth/me").await;
    assert!(!who.text().contains(&first.token) && !who.text().contains("hash"));
}

#[sqlx::test(migrations = false)]
async fn a_failed_login_says_nothing_about_whether_the_username_exists(pool: PgPool) {
    let server = start(&pool).await;
    register(server.addr, "Ann", PASSWORD).await;

    let attempts = [
        ("ann", "the wrong password"),
        ("ANN", "the wrong password"),
        ("nobody_here", PASSWORD),
        ("nobody_here", "the wrong password"),
        ("no", PASSWORD),
        ("an n", PASSWORD),
        ("ann", "short"),
        ("ann", ""),
        ("", ""),
    ];
    let mut replies = Vec::new();
    for (name, password) in attempts {
        let reply = post(
            &server,
            "/api/auth/login",
            &credentials(name, password),
            &[],
        )
        .await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (401, "invalid_credentials"),
            "{name:?} / {password:?}"
        );
        sets_no_cookie(&reply);
        replies.push(reply);
    }
    // The answers are the same down to the byte, so nothing tells the cases apart.
    let first = fingerprint(&replies[0]);
    for reply in &replies[1..] {
        assert_eq!(fingerprint(reply), first);
    }
    assert_eq!(
        count(&pool, "sessions").await,
        1,
        "only the registration's session"
    );
    // The right password under the same spellings still works.
    login(server.addr, "aNn", PASSWORD).await;
}

#[sqlx::test(migrations = false)]
async fn guessing_a_username_is_throttled_and_unknown_names_are_throttled_alike(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.limits = AttemptLimits {
            login_per_username: 3,
            ..roomy_limits()
        };
    })
    .await;
    let addr = server.addr;
    register(addr, "Ann", PASSWORD).await;
    register(addr, "Bob", PASSWORD).await;
    let sessions = count(&pool, "sessions").await;

    for name in ["Ann", "ghost"] {
        for _ in 0..3 {
            let reply = post(
                &server,
                "/api/auth/login",
                &credentials(name, "wrong password!"),
                &[],
            )
            .await;
            assert_eq!(reply.status, 401, "{name}");
        }
        // Over the limit even the right password is refused, as for a name that does not exist.
        let blocked = post(
            &server,
            "/api/auth/login",
            &credentials(name, PASSWORD),
            &[],
        )
        .await;
        assert_eq!(
            (blocked.status, blocked.error_code().as_str()),
            (429, "too_many_attempts"),
            "{name}"
        );
        let wait: u64 = blocked
            .header("retry-after")
            .expect("Retry-After")
            .parse()
            .unwrap();
        assert!((1..=600).contains(&wait), "{wait}");
        sets_no_cookie(&blocked);
        assert!(
            blocked.json()["message"]
                .as_str()
                .unwrap()
                .contains(&wait.to_string())
        );
    }
    assert_eq!(count(&pool, "sessions").await, sessions, "nobody got in");
    // Another account is not affected.
    login(addr, "Bob", PASSWORD).await;
}

#[sqlx::test(migrations = false)]
async fn guessing_from_one_address_is_throttled_behind_a_trusted_proxy(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.trusted_proxies = 1;
        config.auth.limits = AttemptLimits {
            login_per_ip: 3,
            ..roomy_limits()
        };
    })
    .await;
    let forwarded = |ip: &'static str| [("X-Forwarded-For", ip)];
    register(server.addr, "Ann", PASSWORD).await;

    for n in 0..3 {
        let body = credentials(&format!("guess{n}"), "wrong password!");
        let reply = post(&server, "/api/auth/login", &body, &forwarded("203.0.113.1")).await;
        assert_eq!(reply.status, 401);
    }
    // A fourth name, the right password, and a forged prefix all count against the same client.
    for header in ["203.0.113.1", "6.6.6.6, 203.0.113.1", "7.7.7.7,203.0.113.1"] {
        let right = credentials("Ann", PASSWORD);
        let reply = post(&server, "/api/auth/login", &right, &forwarded(header)).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (429, "too_many_attempts"),
            "{header}"
        );
    }
    // Other clients have their own budget.
    let other = post(
        &server,
        "/api/auth/login",
        &credentials("guess9", "wrong password!"),
        &forwarded("203.0.113.2"),
    )
    .await;
    assert_eq!(other.status, 401);
    let fine = post(
        &server,
        "/api/auth/login",
        &credentials("Ann", PASSWORD),
        &forwarded("203.0.113.3"),
    )
    .await;
    assert_eq!(fine.status, 200);
    // Without the header the socket's own address counts, which is a client of its own here.
    let direct = post(
        &server,
        "/api/auth/login",
        &credentials("guess9", "wrong password!"),
        &[],
    )
    .await;
    assert_eq!(direct.status, 401);
}

#[sqlx::test(migrations = false)]
async fn x_forwarded_for_is_ignored_unless_a_proxy_is_trusted(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.limits = AttemptLimits {
            login_per_ip: 3,
            ..roomy_limits()
        };
    })
    .await;
    for n in 0..3 {
        let body = credentials(&format!("guess{n}"), "wrong password!");
        let claimed = format!("198.51.100.{n}");
        let reply = post(
            &server,
            "/api/auth/login",
            &body,
            &[("X-Forwarded-For", &claimed)],
        )
        .await;
        assert_eq!(reply.status, 401);
    }
    // Pretending to be somebody else does not buy a new budget.
    let body = credentials("guess9", "wrong password!");
    let reply = post(
        &server,
        "/api/auth/login",
        &body,
        &[("X-Forwarded-For", "198.51.100.99")],
    )
    .await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (429, "too_many_attempts")
    );
}

#[sqlx::test(migrations = false)]
async fn a_correct_login_does_not_use_up_the_budget(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.limits = AttemptLimits {
            login_per_ip: 2,
            login_per_username: 2,
            ..roomy_limits()
        };
    })
    .await;
    register(server.addr, "Ann", PASSWORD).await;
    for _ in 0..8 {
        login(server.addr, "Ann", PASSWORD).await;
    }
    for expected in [401, 401, 429] {
        let reply = post(
            &server,
            "/api/auth/login",
            &credentials("Ann", "wrong password!"),
            &[],
        )
        .await;
        assert_eq!(reply.status, expected);
    }
}

#[sqlx::test(migrations = false)]
async fn registrations_are_throttled_per_address_and_refused_ones_do_not_count(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.limits = AttemptLimits {
            register_per_ip: 2,
            ..roomy_limits()
        };
    })
    .await;
    for bad in ["x", "has space", "dot.name", "", "ü"] {
        let reply = post(
            &server,
            "/api/auth/register",
            &credentials(bad, PASSWORD),
            &[],
        )
        .await;
        assert_eq!(reply.status, 400);
    }
    for name in ["first", "second"] {
        let reply = post(
            &server,
            "/api/auth/register",
            &credentials(name, PASSWORD),
            &[],
        )
        .await;
        assert_eq!(reply.status, 201);
    }
    let third = post(
        &server,
        "/api/auth/register",
        &credentials("third", PASSWORD),
        &[],
    )
    .await;
    assert_eq!(
        (third.status, third.error_code().as_str()),
        (429, "too_many_attempts")
    );
    assert!(third.header("retry-after").is_some());
    assert_eq!(count(&pool, "users").await, 2);
    // Logging in is a different budget.
    login(server.addr, "first", PASSWORD).await;
}

#[sqlx::test(migrations = false)]
async fn closed_registration_refuses_new_accounts_and_keeps_the_old_ones(pool: PgPool) {
    let open = start(&pool).await;
    let ann = register(open.addr, "Ann", PASSWORD).await;

    let closed = start_with(&pool, |config| {
        config.auth.registration = Registration::Closed;
    })
    .await;
    // Refused before the body is even read, so garbage gets the same answer.
    for body in [
        credentials("Bob", PASSWORD),
        "not json".to_owned(),
        String::new(),
    ] {
        let reply = post(&closed, "/api/auth/register", &body, &[]).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (403, "registration_closed")
        );
        sets_no_cookie(&reply);
    }
    assert_eq!(count(&pool, "users").await, 1);
    // Existing accounts carry on: their sessions, new logins and logouts.
    assert_eq!(me(closed.addr, Some(&ann.cookie)).await.status, 200);
    let again = login(closed.addr, "Ann", PASSWORD).await;
    assert_eq!(
        again.post(closed.addr, "/api/auth/logout", "").await.status,
        204
    );
}

#[sqlx::test(migrations = false)]
async fn a_session_ends_when_it_expires_and_expired_sessions_are_purged(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.purge_every = Duration::from_millis(50);
    })
    .await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let bo = register(addr, "Bob", PASSWORD).await;
    assert_eq!(me(addr, Some(&ann.cookie)).await.status, 200);

    sqlx::query(
        "UPDATE sessions SET expires_at = now() - interval '1 second' WHERE token_digest = $1",
    )
    .bind(Sha256::digest(ann.token.as_bytes()).to_vec())
    .execute(&pool)
    .await
    .unwrap();
    let gone = me(addr, Some(&ann.cookie)).await;
    assert_eq!(
        (gone.status, gone.error_code().as_str()),
        (401, "unauthenticated")
    );
    assert!(gone.headers_named("set-cookie")[0].contains("Max-Age=0"));

    // The janitor deletes the expired row and leaves the live one.
    for _ in 0..200 {
        if count(&pool, "sessions").await == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(count(&pool, "sessions").await, 1);
    assert_eq!(me(addr, Some(&bo.cookie)).await.status, 200);
    // Signing in again works after the old session is gone.
    login(addr, "Ann", PASSWORD).await;
}

#[sqlx::test(migrations = false)]
async fn signing_in_again_replaces_the_session_the_browser_presented(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let before = register(addr, "Ann", PASSWORD).await;
    let other = login(addr, "Ann", PASSWORD).await; // a different browser
    assert_eq!(count(&pool, "sessions").await, 2);

    let body = credentials("Ann", PASSWORD);
    let reply = before.post(addr, "/api/auth/login", &body).await;
    assert_eq!(reply.status, 200);
    let after = session_cookie(&reply).unwrap();
    assert_ne!(after, before.cookie);
    assert_eq!(
        me(addr, Some(&before.cookie)).await.status,
        401,
        "the old session ended"
    );
    assert_eq!(me(addr, Some(&after)).await.status, 200);
    assert_eq!(
        me(addr, Some(&other.cookie)).await.status,
        200,
        "other browsers are left alone"
    );
    assert_eq!(count(&pool, "sessions").await, 2);

    // Registering while signed in does the same for the account it creates.
    let reply = before_register(&server, &after, "Bob").await;
    assert_eq!(reply.status, 201);
    assert_eq!(me(addr, Some(&after)).await.status, 401);
}

async fn before_register(server: &TestServer, cookie: &str, name: &str) -> Reply {
    post(
        server,
        "/api/auth/register",
        &credentials(name, PASSWORD),
        &[("Cookie", cookie)],
    )
    .await
}

#[sqlx::test(migrations = false)]
async fn requests_from_other_sites_change_nothing(pool: PgPool) {
    let listed = "http://localhost:5173";
    let server = start_with(&pool, |config| config.allowed_origins = vec![listed.into()]).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let own_origin = format!("http://{addr}");
    let game = json!({"format": "normal", "seats": [
        {"name": "A", "deck": "mono-green"}, {"name": "B", "deck": "mono-red"}]})
    .to_string();

    for foreign in [
        "https://evil.example",
        "null",
        "http://127.0.0.1:9",
        "http://localhost:5174",
    ] {
        let origin = [("Origin", foreign)];
        let mut with_cookie = vec![("Cookie", ann.cookie.as_str())];
        with_cookie.extend_from_slice(&origin);
        for (path, body, headers) in [
            (
                "/api/auth/register",
                credentials("Eve", PASSWORD),
                &origin[..],
            ),
            ("/api/auth/login", credentials("Ann", PASSWORD), &origin[..]),
            ("/api/auth/logout", String::new(), &with_cookie[..]),
            ("/api/games", game.clone(), &with_cookie[..]),
        ] {
            let reply = post(&server, path, &body, headers).await;
            assert_eq!(
                (reply.status, reply.error_code().as_str()),
                (403, "forbidden_origin"),
                "{foreign} {path}"
            );
            sets_no_cookie(&reply);
        }
    }
    // Nothing happened: no account, no new session, the session still stands, no game.
    assert_eq!(
        (count(&pool, "users").await, count(&pool, "sessions").await),
        (1, 1)
    );
    assert_eq!(me(addr, Some(&ann.cookie)).await.status, 200);
    assert_eq!(server.registry.len(), 0);

    // The server's own pages, a listed front end and plain clients are let through.
    for origin in [Some(own_origin.as_str()), Some(listed), None] {
        let headers: Vec<(&str, &str)> = origin.map(|o| ("Origin", o)).into_iter().collect();
        let reply = post(
            &server,
            "/api/auth/login",
            &credentials("Ann", PASSWORD),
            &headers,
        )
        .await;
        assert_eq!(reply.status, 200, "{origin:?}");
    }
    // Reading is not a change: a GET passes whatever site sent it (the browser's CORS rules hide the answer).
    let read = request(
        addr,
        "GET",
        "/api/auth/me",
        &[("Cookie", &ann.cookie), ("Origin", "https://evil.example")],
        b"",
    )
    .await;
    assert_eq!(read.status, 200);
}

#[sqlx::test(migrations = false)]
async fn a_listed_origin_may_send_the_cookie_with_its_requests(pool: PgPool) {
    let listed = "http://localhost:5173";
    let server = start_with(&pool, |config| config.allowed_origins = vec![listed.into()]).await;
    let reply = post(
        &server,
        "/api/auth/register",
        &credentials("Ann", PASSWORD),
        &[("Origin", listed)],
    )
    .await;
    assert_eq!(reply.status, 201);
    assert_eq!(reply.header("access-control-allow-origin"), Some(listed));
    assert_eq!(
        reply.header("access-control-allow-credentials"),
        Some("true")
    );
    let preflight = request(
        server.addr,
        "OPTIONS",
        "/api/auth/login",
        &[
            ("Origin", listed),
            ("Access-Control-Request-Method", "POST"),
            ("Access-Control-Request-Headers", "content-type"),
        ],
        b"",
    )
    .await;
    assert_eq!(
        preflight.header("access-control-allow-credentials"),
        Some("true")
    );
    // Another site is refused outright, and gets no permission to read an answer either way.
    let stranger = post(
        &server,
        "/api/auth/login",
        &credentials("Ann", PASSWORD),
        &[("Origin", "https://evil.example")],
    )
    .await;
    assert_eq!(stranger.status, 403);
    assert_eq!(stranger.header("access-control-allow-origin"), None);
}

#[tokio::test]
async fn without_a_database_every_account_endpoint_says_so_and_games_still_work() {
    let server = TestServer::start(bot_config()).await;
    let addr = server.addr;
    let stray = "mtg_session=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    for (method, path) in [
        ("POST", "/api/auth/register"),
        ("POST", "/api/auth/login"),
        ("POST", "/api/auth/logout"),
        ("GET", "/api/auth/me"),
    ] {
        let headers = [("Content-Type", "application/json"), ("Cookie", stray)];
        let body = credentials("Ann", PASSWORD);
        let reply = request(addr, method, path, &headers, body.as_bytes()).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (503, "accounts_unavailable"),
            "{method} {path}"
        );
    }
    // A cookie nobody can check is no reason to refuse a guest.
    let headers = [("Content-Type", "application/json"), ("Cookie", stray)];
    let body = net::two_seats().to_string();
    let created = request(addr, "POST", "/api/games", &headers, body.as_bytes()).await;
    assert_eq!(created.status, 201, "{}", created.text());
}

#[sqlx::test(migrations = false)]
async fn a_database_that_fails_gives_a_503_without_details_and_costs_no_throttle_budget(
    pool: PgPool,
) {
    let server = start_with(&pool, |config| {
        config.auth.limits = AttemptLimits {
            login_per_ip: 2,
            login_per_username: 2,
            ..roomy_limits()
        };
    })
    .await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    pool.close().await;

    for _ in 0..6 {
        let reply = post(
            &server,
            "/api/auth/login",
            &credentials("Ann", PASSWORD),
            &[],
        )
        .await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (503, "unavailable")
        );
        let text = reply.text();
        assert!(
            !text.contains("sqlx") && !text.contains("pool") && !text.contains("closed"),
            "{text}"
        );
    }
    let reply = post(
        &server,
        "/api/auth/register",
        &credentials("Bob", PASSWORD),
        &[],
    )
    .await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (503, "unavailable")
    );
    let reply = ann.get(server.addr, "/api/auth/me").await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (503, "unavailable")
    );
}

#[sqlx::test(migrations = false)]
async fn racing_registrations_of_one_name_create_exactly_one_account(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let racers: Vec<_> = (0..8)
        .map(|_| {
            tokio::spawn(async move {
                let body = credentials("Racer", PASSWORD);
                let headers = [("Content-Type", "application/json")];
                request(
                    addr,
                    "POST",
                    "/api/auth/register",
                    &headers,
                    body.as_bytes(),
                )
                .await
                .status
            })
        })
        .collect();
    let mut statuses = Vec::new();
    for racer in racers {
        statuses.push(racer.await.unwrap());
    }
    assert_eq!(
        statuses.iter().filter(|s| **s == 201).count(),
        1,
        "{statuses:?}"
    );
    assert_eq!(
        statuses.iter().filter(|s| **s == 409).count(),
        7,
        "{statuses:?}"
    );
    assert_eq!(
        (count(&pool, "users").await, count(&pool, "sessions").await),
        (1, 1)
    );

    // Many different accounts at once all get through, each with an id of its own.
    let many: Vec<_> = (0..12)
        .map(|n| tokio::spawn(async move { register(addr, &format!("user{n}"), PASSWORD).await }))
        .collect();
    let mut ids = std::collections::BTreeSet::new();
    for task in many {
        ids.insert(task.await.unwrap().id);
    }
    assert_eq!(ids.len(), 12);
}

#[sqlx::test(migrations = false)]
async fn unknown_account_routes_and_wrong_methods_get_small_json_errors(pool: PgPool) {
    let server = start(&pool).await;
    for (method, path, status, code) in [
        ("GET", "/api/auth/login", 405, "method_not_allowed"),
        ("GET", "/api/auth/register", 405, "method_not_allowed"),
        ("GET", "/api/auth/logout", 405, "method_not_allowed"),
        ("POST", "/api/auth/me", 405, "method_not_allowed"),
        ("DELETE", "/api/auth/me", 405, "method_not_allowed"),
        ("GET", "/api/auth", 404, "not_found"),
        ("GET", "/api/auth/admin", 404, "not_found"),
        ("POST", "/api/auth/register/extra", 404, "not_found"),
    ] {
        let reply = request(server.addr, method, path, &[], b"").await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (status, code),
            "{method} {path}"
        );
    }
}

/// Collects everything the process logs, at every level.
#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn captured_logs() -> Arc<Mutex<Vec<u8>>> {
    static LOGS: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    LOGS.get_or_init(|| {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = Capture(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("one subscriber per process");
        buffer
    })
    .clone()
}

#[sqlx::test(migrations = false)]
async fn passwords_and_tokens_never_reach_the_logs(pool: PgPool) {
    let logs = captured_logs();
    let server = start_with(&pool, |config| {
        config.auth.limits = AttemptLimits {
            login_per_username: 2,
            ..roomy_limits()
        };
    })
    .await;
    let password = format!("pw-{}", Uuid::new_v4());
    let wrong = format!("wrong-{}", Uuid::new_v4());
    let username = format!("Log_{}", &Uuid::new_v4().simple().to_string()[..12]);

    let first = register(server.addr, &username, &password).await;
    let second = login(server.addr, &username, &password).await;
    for _ in 0..3 {
        // The last of these is throttled, which logs a warning of its own.
        post(
            &server,
            "/api/auth/login",
            &credentials(&username, &wrong),
            &[],
        )
        .await;
    }
    first.get(server.addr, "/api/auth/me").await;
    me(
        server.addr,
        Some(&format!("mtg_session={}", "ab".repeat(32))),
    )
    .await;
    second.post(server.addr, "/api/auth/logout", "").await;
    let digest_of_first = hex(&Sha256::digest(first.token.as_bytes()));

    let text = String::from_utf8_lossy(&logs.lock().unwrap()).into_owned();
    assert!(text.contains("account created"), "the capture works");
    assert!(
        text.contains("used up its login tries"),
        "the throttle logged"
    );
    assert!(text.contains("turned a client away"), "the throttle logged");
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    for (what, secret) in [
        ("the password", password.as_str()),
        ("a wrong password", wrong.as_str()),
        ("the first token", first.token.as_str()),
        ("the second token", second.token.as_str()),
        ("the first token's digest", digest_of_first.as_str()),
        ("the password hash", hash.as_str()),
        (
            "the hash's salt and output",
            hash.rsplit('$').next().unwrap(),
        ),
    ] {
        assert!(!text.contains(secret), "the logs contain {what}");
    }
    // Failed attempts are logged without the name that was tried.
    assert!(
        !text.contains(&username),
        "the logs name the account that was tried"
    );
}
