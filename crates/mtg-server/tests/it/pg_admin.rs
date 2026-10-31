//! Goal: the admin account and nobody else makes accounts: the start-up sync marks the account `MTG_ADMIN_USERNAME` names and clears every other flag; `POST /api/admin/users` and `GET /api/admin/users` answer 503, 401 and 403 in that order before the body is read, read the flag from the database on every request, make an account without signing anyone in whether registration is open or closed, follow the registration rules and errors, keep a throttle of their own, and list accounts without anything that opens them; `me` carries the flag, `GET /api/auth/options` tells a visitor whether registering is open, and no password reaches the logs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;

use std::net::SocketAddr;
use std::time::Duration;

use mtg_server::config::{AttemptLimits, Registration};
use mtg_server::{ServerConfig, migrate};
use net::{
    Browser, PASSWORD, Reply, TestServer, bot_config, credentials, login, me, register, request,
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const ADMIN: &str = "Chrisser";

/// Limits no test trips by accident; the throttle tests set their own.
pub(crate) fn roomy_limits() -> AttemptLimits {
    AttemptLimits {
        window: Duration::from_secs(600),
        login_per_ip: 10_000,
        login_per_username: 10_000,
        register_per_ip: 10_000,
        admin_creates_per_account: 10_000,
        code_window: Duration::from_secs(600),
        code_misses_per_ip: 10_000,
        lobby_window: Duration::from_secs(600),
        lobby_requests_per_ip: 10_000,
        precon_requests_per_ip: 10_000,
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

/// A server whose admin is `admin` (as the setting spells it).
async fn start_admin(pool: &PgPool, admin: Option<&str>) -> TestServer {
    start_with(pool, |config| config.auth.admin = admin.map(str::to_owned)).await
}

/// A server on a database where `ADMIN` and a friend already have accounts, with `ADMIN` the admin
/// (the account has to exist before the server starts for the setting to find it).
async fn world(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> (TestServer, Browser) {
    let first = start_admin(pool, None).await;
    register(first.addr, ADMIN, PASSWORD).await;
    register(first.addr, "Friend_1", PASSWORD).await;
    drop(first);
    let server = start_with(pool, |config| {
        config.auth.admin = Some(ADMIN.to_owned());
        change(config);
    })
    .await;
    let admin = login(server.addr, ADMIN, PASSWORD).await;
    (server, admin)
}

async fn post(addr: SocketAddr, path: &str, body: &str, extra: &[(&str, &str)]) -> Reply {
    let mut headers = vec![("Content-Type", "application/json")];
    headers.extend_from_slice(extra);
    request(addr, "POST", path, &headers, body.as_bytes()).await
}

/// `POST /api/admin/users` with any body and content type, with a session cookie or without.
async fn post_with(
    addr: SocketAddr,
    body: &str,
    content_type: &str,
    cookie: Option<&str>,
) -> Reply {
    let mut headers = vec![("Content-Type", content_type)];
    headers.extend(cookie.map(|cookie| ("Cookie", cookie)));
    request(addr, "POST", "/api/admin/users", &headers, body.as_bytes()).await
}

async fn make(by: &Browser, server: &TestServer, name: &str, password: &str) -> Reply {
    by.post(
        server.addr,
        "/api/admin/users",
        &credentials(name, password),
    )
    .await
}

pub(crate) async fn count(pool: &PgPool, table: &str) -> i64 {
    // `table` is always one of this file's own literals.
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The usernames that are admin, sorted.
pub(crate) async fn admins(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT username FROM users WHERE is_admin ORDER BY username")
        .fetch_all(pool)
        .await
        .unwrap()
}

/// The reply sets no cookie at all.
pub(crate) fn sets_no_cookie(reply: &Reply) {
    let lines = reply.headers_named("set-cookie");
    assert_eq!(lines.len(), 0, "unexpected cookies: {lines:?}");
}

pub(crate) fn error_of(reply: &Reply) -> (u16, String) {
    (reply.status, reply.error_code())
}

#[sqlx::test(migrations = false)]
async fn the_account_the_setting_names_is_the_admin_and_the_answers_say_so(pool: PgPool) {
    let first = start_admin(&pool, None).await;
    let registered = register(first.addr, "chrisser", PASSWORD).await;
    register(first.addr, "Friend_1", PASSWORD).await;
    // Nobody is admin without the setting, and the answers say so.
    let who = registered.get(first.addr, "/api/auth/me").await;
    assert_eq!(who.json()["is_admin"], json!(false));
    drop(first);

    // The setting is compared ignoring case, and a restart marks the account.
    let server = start_admin(&pool, Some("ChrisSer")).await;
    assert_eq!(admins(&pool).await, ["chrisser"]);
    let admin = login(server.addr, "CHRISSER", PASSWORD).await;
    let who = admin.get(server.addr, "/api/auth/me").await;
    assert_eq!(
        who.json(),
        json!({"id": admin.id, "username": "chrisser", "is_admin": true})
    );
    let friend = login(server.addr, "friend_1", PASSWORD).await;
    let who = friend.get(server.addr, "/api/auth/me").await;
    assert_eq!(
        who.json(),
        json!({"id": friend.id, "username": "Friend_1", "is_admin": false})
    );
    // Signing in and registering answer with the flag as well.
    let body = credentials("chrisser", PASSWORD);
    let again = post(server.addr, "/api/auth/login", &body, &[]).await;
    assert_eq!(again.json()["is_admin"], json!(true));
    let body = credentials("Newcomer", PASSWORD);
    let new = post(server.addr, "/api/auth/register", &body, &[]).await;
    assert_eq!(new.status, 201);
    assert_eq!(new.json()["is_admin"], json!(false));
}

#[sqlx::test(migrations = false)]
async fn the_start_up_sync_sets_one_admin_and_clears_the_rest(pool: PgPool) {
    let first = start_admin(&pool, None).await;
    for name in ["Ann_1", "Bob_2", "Cy_3"] {
        register(first.addr, name, PASSWORD).await;
    }
    drop(first);
    assert!(admins(&pool).await.is_empty(), "nobody without a setting");

    // (setting, who is admin after the start)
    let steps: [(Option<&str>, &[&str]); 8] = [
        (Some("ann_1"), &["Ann_1"]),
        (Some("Ann_1"), &["Ann_1"]),
        // A rename demotes the old admin in the same statement.
        (Some("BOB_2"), &["Bob_2"]),
        // Removing the setting demotes the admin.
        (None, &[]),
        (Some("cy_3"), &["Cy_3"]),
        // A name that belongs to no account promotes nobody, and the old admin is demoted.
        (Some("nobody_here"), &[]),
        // With nobody to demote it changes nothing and the server still starts.
        (Some("nobody_here"), &[]),
        (Some("ann_1"), &["Ann_1"]),
    ];
    for (setting, expected) in steps {
        let server = start_admin(&pool, setting).await;
        assert_eq!(admins(&pool).await, expected, "setting {setting:?}");
        assert_eq!(count(&pool, "users").await, 3);
        drop(server);
    }
}

#[sqlx::test(migrations = false)]
async fn nobody_can_take_the_admins_name_in_another_case(pool: PgPool) {
    let (server, admin) = world(&pool, |_| {}).await;
    for name in ["Chrisser", "chrisser", "CHRISSER"] {
        let reply = make(&admin, &server, name, PASSWORD).await;
        assert_eq!(error_of(&reply), (409, "username_taken".into()), "{name}");
    }
    assert_eq!(admins(&pool).await, [ADMIN]);
}

#[tokio::test]
async fn without_a_database_the_routes_answer_503_first() {
    let server = TestServer::start(bot_config()).await;
    let addr = server.addr;
    let cookie = format!("mtg_session={}", "ab".repeat(32));
    for extra in [vec![], vec![("Cookie", cookie.as_str())]] {
        let created = post(addr, "/api/admin/users", "not json", &extra).await;
        assert_eq!(error_of(&created), (503, "accounts_unavailable".into()));
        let listed = request(addr, "GET", "/api/admin/users", &extra, b"").await;
        assert_eq!(error_of(&listed), (503, "accounts_unavailable".into()));
    }
    let options = request(addr, "GET", "/api/auth/options", &[], b"").await;
    assert_eq!(error_of(&options), (503, "accounts_unavailable".into()));
}

#[sqlx::test(migrations = false)]
async fn nobody_but_the_admin_gets_further_than_the_checks_whatever_the_body(pool: PgPool) {
    let (server, admin) = world(&pool, |_| {}).await;
    let addr = server.addr;
    let friend = login(addr, "Friend_1", PASSWORD).await;
    let before = count(&pool, "users").await;
    let stale = format!("mtg_session={}", "cd".repeat(32));
    let good = r#"{"username":"Newcomer","password":"correct horse battery"}"#;
    let extra = r#"{"username":"Newcomer","password":"correct horse battery","admin":true}"#;
    let cases: [(&str, &str); 6] = [
        ("this is not json", "application/json"),
        ("", "application/json"),
        (good, "text/plain"),
        (extra, "application/json"),
        (r#"{"username":"Newcomer"}"#, "application/json"),
        (r#"{"username":"x","password":"y"}"#, "application/json"),
    ];
    for (body, content_type) in cases {
        // 401 for no session and for one the server does not know, which is told to go.
        let nobody = post_with(addr, body, content_type, None).await;
        assert_eq!(error_of(&nobody), (401, "unauthenticated".into()), "{body}");
        sets_no_cookie(&nobody);
        let unknown = post_with(addr, body, content_type, Some(&stale)).await;
        assert_eq!(
            error_of(&unknown),
            (401, "unauthenticated".into()),
            "{body}"
        );
        let told = unknown.headers_named("set-cookie");
        assert!(told.len() == 1 && told[0].contains("Max-Age=0"), "{told:?}");

        // 403 for a signed-in account that is not the admin, decided before the body is read.
        let refused = post_with(addr, body, content_type, Some(&friend.cookie)).await;
        assert_eq!(error_of(&refused), (403, "forbidden".into()), "{body}");
        sets_no_cookie(&refused);

        // The admin gets the verdict on the body: never a 401 or a 403.
        let judged = post_with(addr, body, content_type, Some(&admin.cookie)).await;
        assert!(
            matches!(judged.status, 400 | 415),
            "{} {}",
            judged.status,
            judged.text()
        );
    }
    assert_eq!(count(&pool, "users").await, before);
}

#[sqlx::test(migrations = false)]
async fn a_huge_body_from_a_non_admin_is_forbidden_not_too_large(pool: PgPool) {
    let (server, _admin) = world(&pool, |_| {}).await;
    let friend = login(server.addr, "Friend_1", PASSWORD).await;
    let huge = "x".repeat(1_000_000);
    let refused = friend.post(server.addr, "/api/admin/users", &huge).await;
    assert_eq!(error_of(&refused), (403, "forbidden".into()));
}

#[sqlx::test(migrations = false)]
async fn the_list_answers_401_and_403_before_anything_else(pool: PgPool) {
    let (server, _admin) = world(&pool, |_| {}).await;
    let addr = server.addr;
    let friend = login(addr, "Friend_1", PASSWORD).await;
    let nobody = request(addr, "GET", "/api/admin/users", &[], b"").await;
    assert_eq!(error_of(&nobody), (401, "unauthenticated".into()));
    let refused = friend.get(addr, "/api/admin/users").await;
    assert_eq!(error_of(&refused), (403, "forbidden".into()));
    assert!(!refused.text().contains("Chrisser"), "{}", refused.text());
}

/// The admin makes an account under `registration` and nobody is signed in by it.
async fn admin_makes_an_account(pool: PgPool, registration: Registration) {
    let (server, admin) = world(&pool, |config| config.auth.registration = registration).await;
    let addr = server.addr;
    let sessions = count(&pool, "sessions").await;

    let password = format!("pw-{}", Uuid::new_v4());
    let made = make(&admin, &server, "Newcomer_7", &password).await;
    assert_eq!(made.status, 201, "{}", made.text());
    let body = made.json();
    let mut keys: Vec<_> = body.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["id", "username"]);
    assert_eq!(body["username"], json!("Newcomer_7"));
    assert!(body["id"].as_str().unwrap().parse::<Uuid>().is_ok());
    // No session for the admin's browser and none for the new account.
    sets_no_cookie(&made);
    assert_eq!(count(&pool, "sessions").await, sessions);
    assert_eq!(made.header("cache-control"), Some("no-store"));

    // The account exists, works with the password, and is not an admin.
    let newcomer = login(addr, "newcomer_7", &password).await;
    assert_eq!(newcomer.id, body["id"].as_str().unwrap());
    let who = newcomer.get(addr, "/api/auth/me").await;
    assert_eq!(who.json()["is_admin"], json!(false));
    assert_eq!(admins(&pool).await, [ADMIN]);
    // Only the hash is stored.
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE username = $1")
        .bind("Newcomer_7")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$") && !hash.contains(&password));

    // Registering stays as it was: closed refuses, open signs in.
    let body = credentials("Selfmade_8", PASSWORD);
    let registered = post(addr, "/api/auth/register", &body, &[]).await;
    match registration {
        Registration::Closed => {
            assert_eq!(error_of(&registered), (403, "registration_closed".into()));
        }
        Registration::Open => assert_eq!(registered.status, 201),
    }
}

#[sqlx::test(migrations = false)]
async fn the_admin_makes_an_account_while_registration_is_closed(pool: PgPool) {
    admin_makes_an_account(pool, Registration::Closed).await;
}

#[sqlx::test(migrations = false)]
async fn the_admin_makes_an_account_while_registration_is_open(pool: PgPool) {
    admin_makes_an_account(pool, Registration::Open).await;
}

#[sqlx::test(migrations = false)]
async fn the_admin_route_follows_the_rules_and_errors_of_registering(pool: PgPool) {
    let (server, admin) = world(&pool, |_| {}).await;
    let (long_name, ok_password) = ("0123456789012345678901234567890a", "correct horse battery");
    let too_long_name = format!("{long_name}b");
    let long_password = "p".repeat(128);
    let too_long_password = "p".repeat(129);
    // (name, password, status, error code)
    let cases: Vec<(&str, &str, u16, &str)> = vec![
        ("ab", ok_password, 400, "invalid_username"),
        (&too_long_name, ok_password, 400, "invalid_username"),
        ("new comer", ok_password, 400, "invalid_username"),
        ("newcömer", ok_password, 400, "invalid_username"),
        (" Newcomer", ok_password, 400, "invalid_username"),
        ("Newcomer", "123456789", 400, "invalid_password"),
        ("Newcomer", &too_long_password, 400, "invalid_password"),
        ("Friend_1", ok_password, 409, "username_taken"),
        ("FRIEND_1", ok_password, 409, "username_taken"),
        ("abc", "0123456789", 201, ""),
        (long_name, &long_password, 201, ""),
    ];
    for (name, password, status, code) in cases {
        let before = count(&pool, "users").await;
        let reply = make(&admin, &server, name, password).await;
        assert_eq!(reply.status, status, "{name}: {}", reply.text());
        if status == 201 {
            assert_eq!(count(&pool, "users").await, before + 1);
            login(server.addr, name, password).await;
            continue;
        }
        assert_eq!(reply.error_code(), code, "{name}");
        assert_eq!(count(&pool, "users").await, before);
        // The message states the rule and never echoes what was sent.
        let message = reply.json()["message"].as_str().unwrap().to_owned();
        assert!(!message.contains(password), "{message}");
        // The same body gets the same verdict from the registration route.
        let registered = post(
            server.addr,
            "/api/auth/register",
            &credentials(name, password),
            &[],
        )
        .await;
        assert_eq!(error_of(&registered), (status, code.to_owned()), "{name}");
        assert_eq!(registered.json()["message"], json!(message));
    }
}

#[sqlx::test(migrations = false)]
async fn the_admins_budget_is_its_own_and_costs_the_address_nothing(pool: PgPool) {
    let (server, admin) = world(&pool, |config| {
        config.auth.limits = AttemptLimits {
            admin_creates_per_account: 2,
            register_per_ip: 1,
            ..roomy_limits()
        };
    })
    .await;
    let addr = server.addr;
    // Tries the rules refuse cost no hashing and no budget.
    for _ in 0..5 {
        assert_eq!(make(&admin, &server, "x", PASSWORD).await.status, 400);
        assert_eq!(make(&admin, &server, "Newcomer", "short").await.status, 400);
    }
    assert_eq!(make(&admin, &server, "Made_1", PASSWORD).await.status, 201);
    // A taken name was hashed, so it counts.
    assert_eq!(
        make(&admin, &server, "Friend_1", PASSWORD).await.status,
        409
    );
    let blocked = make(&admin, &server, "Made_2", PASSWORD).await;
    assert_eq!(error_of(&blocked), (429, "too_many_attempts".into()));
    let wait: u64 = blocked.header("retry-after").unwrap().parse().unwrap();
    assert!((1..=600).contains(&wait), "{wait}");
    let made: Option<String> =
        sqlx::query_scalar("SELECT username FROM users WHERE username = 'Made_2'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(made, None, "a blocked try makes nothing");
    // The list is not throttled by it, and a registration still has its own address budget.
    assert_eq!(admin.get(addr, "/api/admin/users").await.status, 200);
    let body = credentials("Selfmade_1", PASSWORD);
    assert_eq!(
        post(addr, "/api/auth/register", &body, &[]).await.status,
        201
    );
    let body = credentials("Selfmade_2", PASSWORD);
    let second = post(addr, "/api/auth/register", &body, &[]).await;
    assert_eq!(error_of(&second), (429, "too_many_attempts".into()));
}

#[sqlx::test(migrations = false)]
async fn registrations_from_an_address_do_not_use_up_the_admins_budget(pool: PgPool) {
    let (server, admin) = world(&pool, |config| {
        config.auth.limits = AttemptLimits {
            admin_creates_per_account: 3,
            register_per_ip: 2,
            ..roomy_limits()
        };
    })
    .await;
    for n in 0..2 {
        let body = credentials(&format!("Self_{n}"), PASSWORD);
        let reply = post(server.addr, "/api/auth/register", &body, &[]).await;
        assert_eq!(reply.status, 201);
    }
    let body = credentials("Self_9", PASSWORD);
    let over = post(server.addr, "/api/auth/register", &body, &[]).await;
    assert_eq!(over.status, 429);
    for n in 0..3 {
        let name = format!("Made_{n}");
        assert_eq!(make(&admin, &server, &name, PASSWORD).await.status, 201);
    }
}

#[sqlx::test(migrations = false)]
async fn the_list_is_oldest_first_and_holds_nothing_that_opens_an_account(pool: PgPool) {
    let (server, admin) = world(&pool, |_| {}).await;
    let addr = server.addr;
    let secret = format!("pw-{}", Uuid::new_v4());
    for name in ["Zed_1", "Amy_2", "Made_3"] {
        assert_eq!(make(&admin, &server, name, &secret).await.status, 201);
    }

    let reply = admin.get(addr, "/api/admin/users").await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    let body = reply.json();
    assert_eq!(body.as_object().unwrap().len(), 1);
    let users = body["users"].as_array().unwrap();
    let names: Vec<_> = users
        .iter()
        .map(|user| user["username"].as_str().unwrap())
        .collect();
    assert_eq!(names, [ADMIN, "Friend_1", "Zed_1", "Amy_2", "Made_3"]);
    for user in users {
        let mut keys: Vec<_> = user
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["created_at", "id", "is_admin", "username"]);
        let at = user["created_at"].as_str().unwrap();
        assert!(
            at.len() == 20 && at.ends_with('Z') && at.as_bytes()[10] == b'T',
            "{at}"
        );
        assert!(user["id"].as_str().unwrap().parse::<Uuid>().is_ok());
    }
    let flags: Vec<_> = users
        .iter()
        .map(|user| user["is_admin"].as_bool().unwrap())
        .collect();
    assert_eq!(flags, [true, false, false, false, false]);
    let times: Vec<_> = users
        .iter()
        .map(|user| user["created_at"].as_str().unwrap())
        .collect();
    assert!(times.is_sorted(), "{times:?}");

    // Nothing that opens an account, however it is spelled.
    let text = reply.text();
    let hashes: Vec<String> = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!hashes.is_empty() && count(&pool, "sessions").await > 0);
    for hash in &hashes {
        assert!(!text.contains(hash.as_str()));
    }
    for needle in [
        "argon2",
        "hash",
        "password",
        "session",
        "token",
        "digest",
        "cookie",
        secret.as_str(),
        admin.token.as_str(),
    ] {
        assert!(!text.to_lowercase().contains(needle), "{needle} in {text}");
    }
}

#[sqlx::test(migrations = false)]
async fn the_flag_is_read_from_the_database_on_every_request(pool: PgPool) {
    let (server, admin) = world(&pool, |_| {}).await;
    let addr = server.addr;
    let friend = login(addr, "Friend_1", PASSWORD).await;
    assert_eq!(admin.get(addr, "/api/admin/users").await.status, 200);
    assert_eq!(friend.get(addr, "/api/admin/users").await.status, 403);

    // The same cookies, a different row: the verdict follows the database at once.
    sqlx::query("UPDATE users SET is_admin = (username = 'Friend_1')")
        .execute(&pool)
        .await
        .unwrap();
    let demoted = admin.get(addr, "/api/admin/users").await;
    assert_eq!(error_of(&demoted), (403, "forbidden".into()));
    assert_eq!(make(&admin, &server, "Late_1", PASSWORD).await.status, 403);
    let who = admin.get(addr, "/api/auth/me").await;
    assert_eq!(who.json()["is_admin"], json!(false));
    assert_eq!(friend.get(addr, "/api/admin/users").await.status, 200);
    assert_eq!(make(&friend, &server, "Late_2", PASSWORD).await.status, 201);
    let who = friend.get(addr, "/api/auth/me").await;
    assert_eq!(who.json()["is_admin"], json!(true));
}

#[sqlx::test(migrations = false)]
async fn a_foreign_site_cannot_make_accounts_with_the_admins_cookie(pool: PgPool) {
    let (server, admin) = world(&pool, |_| {}).await;
    let before = count(&pool, "users").await;
    let reply = admin
        .send(
            server.addr,
            "POST",
            "/api/admin/users",
            &credentials("Evil_1", PASSWORD),
            &[("Origin", "https://evil.example")],
        )
        .await;
    assert_eq!(error_of(&reply), (403, "forbidden_origin".into()));
    assert_eq!(count(&pool, "users").await, before);
}

/// `GET /api/auth/options` under `registration` says `open`.
async fn options_say(pool: PgPool, registration: Registration, open: bool) {
    let server = start_with(&pool, |config| config.auth.registration = registration).await;
    let reply = request(server.addr, "GET", "/api/auth/options", &[], b"").await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.json(), json!({"registration_open": open}));
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    // Asking needs no session and changes nothing.
    assert_eq!(me(server.addr, None).await.status, 401);
}

#[sqlx::test(migrations = false)]
async fn a_visitor_is_told_registering_is_open(pool: PgPool) {
    options_say(pool, Registration::Open, true).await;
}

#[sqlx::test(migrations = false)]
async fn a_visitor_is_told_registering_is_closed(pool: PgPool) {
    options_say(pool, Registration::Closed, false).await;
}

#[sqlx::test(migrations = false)]
async fn the_passwords_of_accounts_the_admin_makes_never_reach_the_logs(pool: PgPool) {
    let logs = common::capture_logs();
    let (server, admin) = world(&pool, |_| {}).await;
    let password = format!("pw-{}", Uuid::new_v4());
    let wrong = format!("wrong-{}", Uuid::new_v4());
    let name = format!("Log_{}", &Uuid::new_v4().simple().to_string()[..12]);
    assert_eq!(make(&admin, &server, &name, &password).await.status, 201);
    // A taken name, a refused try and a refused caller are logged without what was typed.
    assert_eq!(make(&admin, &server, &name, &wrong).await.status, 409);
    assert_eq!(make(&admin, &server, "x", &wrong).await.status, 400);
    let friend = login(server.addr, "Friend_1", PASSWORD).await;
    assert_eq!(make(&friend, &server, "Evil_9", &wrong).await.status, 403);

    let logged = String::from_utf8_lossy(&logs.lock().unwrap()).into_owned();
    assert!(logged.contains("account created by the admin"));
    for secret in [
        password.as_str(),
        wrong.as_str(),
        admin.token.as_str(),
        friend.token.as_str(),
    ] {
        assert!(!logged.contains(secret), "a secret reached the logs");
    }
}
