//! Goal: the admin sets a friend's password and removes a friend's account over real HTTP on Postgres: both routes answer 503, 401 and 403 in that order before the body is read, then 404 for an id that names no account and 409 for the admin's own account; a password set by the admin follows the creation rules, ends every session of the account and is never logged; removal is the deletion of `DELETE /api/auth/account` (decks, sessions, seats, the lobby the account hosts, a free username afterwards); both routes use the admin's budget; the admin cannot delete its own account in Account settings either; and a body that does not parse gets one fixed plain answer on every credential route.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;
use crate::pg_account_settings::{make_as, mentions, seat_row, status_of};
use crate::pg_admin::{admins, count, error_of, roomy_limits, sets_no_cookie};

use std::net::SocketAddr;

use mtg_db::{Database, DeckDraft};
use mtg_server::config::AttemptLimits;
use mtg_server::{ServerConfig, migrate};
use net::lobby::{self, join};
use net::{
    Browser, PASSWORD, Reply, TestServer, bot_config, credentials, login, register, request,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

const ADMIN: &str = "Chrisser";
const FIXED_400: &str = "the body is not a valid request: send a JSON object with exactly the fields this request needs";

/// Both schemas migrated behind a running server, after `change` has adjusted the configuration.
async fn start_with(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> TestServer {
    migrate(pool).await.unwrap();
    Database::from_pool(pool.clone()).migrate().await.unwrap();
    let mut config = bot_config();
    config.auth.limits = roomy_limits();
    change(&mut config);
    TestServer::start_on(pool.clone(), config).await
}

/// A server whose admin is `ADMIN`, with that account, `Friend_1` and `Other_2` signed in once.
struct World {
    server: TestServer,
    admin: Browser,
    friend: Browser,
    other: Browser,
}

async fn world(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> World {
    let first = start_with(pool, |_| {}).await;
    for name in [ADMIN, "Friend_1", "Other_2"] {
        register(first.addr, name, PASSWORD).await;
    }
    drop(first);
    let server = start_with(pool, |config| {
        config.auth.admin = Some(ADMIN.to_owned());
        change(config);
    })
    .await;
    let addr = server.addr;
    World {
        admin: login(addr, ADMIN, PASSWORD).await,
        friend: login(addr, "Friend_1", PASSWORD).await,
        other: login(addr, "Other_2", PASSWORD).await,
        server,
    }
}

fn password_path(id: &str) -> String {
    format!("/api/admin/users/{id}/password")
}

fn remove_path(id: &str) -> String {
    format!("/api/admin/users/{id}")
}

async fn set_password(by: &Browser, server: &TestServer, id: &str, password: &str) -> Reply {
    let body = json!({ "password": password }).to_string();
    by.post(server.addr, &password_path(id), &body).await
}

async fn remove(by: &Browser, server: &TestServer, id: &str) -> Reply {
    by.delete(server.addr, &remove_path(id)).await
}

async fn make(by: &Browser, server: &TestServer, name: &str, password: &str) -> Reply {
    by.post(
        server.addr,
        "/api/admin/users",
        &credentials(name, password),
    )
    .await
}

async fn hash_of(pool: &PgPool, id: &str) -> String {
    sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn sessions_of(pool: &PgPool, id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sessions WHERE user_id = $1")
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn raw(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: &str,
    content_type: &str,
    cookie: Option<&str>,
) -> Reply {
    let mut headers = vec![("Content-Type", content_type)];
    headers.extend(cookie.map(|cookie| ("Cookie", cookie)));
    request(addr, method, path, &headers, body.as_bytes()).await
}

#[tokio::test]
async fn without_a_database_the_new_routes_answer_503_first() {
    let server = TestServer::start(bot_config()).await;
    let cookie = format!("mtg_session={}", "ab".repeat(32));
    let id = Uuid::new_v4().to_string();
    for cookie in [None, Some(cookie.as_str())] {
        for path in [password_path(&id), password_path("not-an-id")] {
            let reply = raw(server.addr, "POST", &path, "not json", "text/plain", cookie).await;
            assert_eq!(error_of(&reply), (503, "accounts_unavailable".into()));
        }
        for path in [remove_path(&id), remove_path("not-an-id")] {
            let reply = raw(server.addr, "DELETE", &path, "", "application/json", cookie).await;
            assert_eq!(error_of(&reply), (503, "accounts_unavailable".into()));
        }
    }
}

#[sqlx::test(migrations = false)]
async fn nobody_but_the_admin_gets_past_the_checks_whatever_the_id_and_the_body(pool: PgPool) {
    let w = world(&pool, |_| {}).await;
    let addr = w.server.addr;
    let stale = format!("mtg_session={}", "cd".repeat(32));
    let (users, hash) = (
        count(&pool, "users").await,
        hash_of(&pool, &w.friend.id).await,
    );
    let good = r#"{"password":"correct horse battery"}"#;
    let cases = [
        ("this is not json", "application/json"),
        ("", "application/json"),
        (good, "text/plain"),
        (r#"{"password":"x"}"#, "application/json"),
        (good, "application/json"),
    ];
    // The ids do not matter either: a real one, the admin's, an unknown one and a malformed one.
    let ids = [
        w.friend.id.clone(),
        w.admin.id.clone(),
        Uuid::new_v4().to_string(),
        "not-an-id".to_owned(),
    ];
    for id in &ids {
        for (body, content_type) in cases {
            for (method, path) in [("POST", password_path(id)), ("DELETE", remove_path(id))] {
                let nobody = raw(addr, method, &path, body, content_type, None).await;
                assert_eq!(error_of(&nobody), (401, "unauthenticated".into()), "{path}");
                sets_no_cookie(&nobody);
                let unknown = raw(addr, method, &path, body, content_type, Some(&stale)).await;
                assert_eq!(error_of(&unknown), (401, "unauthenticated".into()));
                let told = unknown.headers_named("set-cookie");
                assert!(told.len() == 1 && told[0].contains("Max-Age=0"), "{told:?}");

                for who in [&w.friend, &w.other] {
                    let refused = raw(addr, method, &path, body, content_type, Some(&who.cookie));
                    let refused = refused.await;
                    assert_eq!(error_of(&refused), (403, "forbidden".into()), "{path}");
                    sets_no_cookie(&refused);
                }
            }
        }
    }
    // A huge body from an account that is not the admin is forbidden, not too large.
    let huge = "x".repeat(1_000_000);
    let big = w
        .friend
        .post(addr, &password_path(&w.other.id), &huge)
        .await;
    assert_eq!(error_of(&big), (403, "forbidden".into()));

    assert_eq!(count(&pool, "users").await, users);
    assert_eq!(hash_of(&pool, &w.friend.id).await, hash);
}

#[sqlx::test(migrations = false)]
async fn an_unknown_id_is_404_and_the_admins_own_account_is_409_before_the_body_is_read(
    pool: PgPool,
) {
    let w = world(&pool, |_| {}).await;
    let addr = w.server.addr;
    let (users, hash) = (
        count(&pool, "users").await,
        hash_of(&pool, &w.admin.id).await,
    );
    let unknown = [
        Uuid::new_v4().to_string(),
        "not-an-id".to_owned(),
        "12345".to_owned(),
        Uuid::nil().to_string(),
    ];
    for id in &unknown {
        for body in ["not json", r#"{"password":"correct horse battery"}"#] {
            let reply = w.admin.post(addr, &password_path(id), body).await;
            assert_eq!(error_of(&reply), (404, "unknown_account".into()), "{id}");
        }
        let reply = remove(&w.admin, &w.server, id).await;
        assert_eq!(error_of(&reply), (404, "unknown_account".into()), "{id}");
        assert_eq!(reply.json()["message"], json!("there is no such account"));
    }

    // The admin's own account: the password changes in Account settings, the account stays.
    for body in ["not json", "", r#"{"password":"correct horse battery"}"#] {
        let reply = w.admin.post(addr, &password_path(&w.admin.id), body).await;
        assert_eq!(error_of(&reply), (409, "admin_account".into()), "{body}");
        let message = reply.json()["message"].as_str().unwrap().to_owned();
        assert!(message.contains("Account settings"), "{message}");
    }
    let own = remove(&w.admin, &w.server, &w.admin.id).await;
    assert_eq!(error_of(&own), (409, "admin_account".into()));
    assert_eq!(
        own.json()["message"],
        json!("this account makes accounts for your friends, so it cannot be deleted")
    );
    sets_no_cookie(&own);

    assert_eq!(count(&pool, "users").await, users);
    assert_eq!(hash_of(&pool, &w.admin.id).await, hash);
    assert_eq!(admins(&pool).await, [ADMIN]);
    assert_eq!(w.admin.get(addr, "/api/auth/me").await.status, 200);
}

#[sqlx::test(migrations = false)]
async fn a_password_the_admin_sets_ends_every_session_of_the_account(pool: PgPool) {
    let w = world(&pool, |_| {}).await;
    let addr = w.server.addr;
    let phone = login(addr, "Friend_1", PASSWORD).await;
    // Three: the registration, the sign-in of the world and the phone.
    assert_eq!(sessions_of(&pool, &w.friend.id).await, 3);
    let other_hash = hash_of(&pool, &w.other.id).await;
    let before = hash_of(&pool, &w.friend.id).await;

    let fresh = format!("pw-{}", Uuid::new_v4());
    let reply = set_password(&w.admin, &w.server, &w.friend.id, &fresh).await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    assert_eq!(reply.body.len(), 0);
    sets_no_cookie(&reply);
    assert_eq!(reply.header("cache-control"), Some("no-store"));

    // Both sessions of the friend are gone; the admin's and the other account's stay.
    assert_eq!(sessions_of(&pool, &w.friend.id).await, 0);
    for session in [&w.friend, &phone] {
        let me = session.get(addr, "/api/auth/me").await;
        assert_eq!(error_of(&me), (401, "unauthenticated".into()));
    }
    assert_eq!(w.admin.get(addr, "/api/auth/me").await.status, 200);
    assert_eq!(w.other.get(addr, "/api/auth/me").await.status, 200);

    // The old password is dead and the new one works; only the hash is stored.
    let old = request(
        addr,
        "POST",
        "/api/auth/login",
        &[("Content-Type", "application/json")],
        credentials("Friend_1", PASSWORD).as_bytes(),
    )
    .await;
    assert_eq!(error_of(&old), (401, "invalid_credentials".into()));
    let back = login(addr, "friend_1", &fresh).await;
    assert_eq!(back.id, w.friend.id);
    let hash = hash_of(&pool, &w.friend.id).await;
    assert!(hash.starts_with("$argon2id$") && !hash.contains(&fresh) && hash != before);
    assert_eq!(hash_of(&pool, &w.other.id).await, other_hash);
    assert_eq!(admins(&pool).await, [ADMIN]);

    // The id may be spelled in capitals, and the same password may be set again.
    let again = set_password(&w.admin, &w.server, &w.friend.id.to_uppercase(), &fresh).await;
    assert_eq!(again.status, 204, "{}", again.text());
}

#[sqlx::test(migrations = false)]
async fn a_password_the_admin_sets_follows_the_creation_rules(pool: PgPool) {
    let w = world(&pool, |_| {}).await;
    let addr = w.server.addr;
    let before = hash_of(&pool, &w.friend.id).await;
    let path = password_path(&w.friend.id);
    let rule = {
        let reply = make(&w.admin, &w.server, "Newcomer", "short").await;
        reply.json()["message"].as_str().unwrap().to_owned()
    };

    for password in ["123456789", &"p".repeat(129), ""] {
        let reply = set_password(&w.admin, &w.server, &w.friend.id, password).await;
        assert_eq!(
            error_of(&reply),
            (400, "invalid_password".into()),
            "{password}"
        );
        let message = reply.json()["message"].as_str().unwrap().to_owned();
        assert_eq!(message, rule);
        assert!(
            password.is_empty() || !message.contains(password),
            "{message}"
        );
    }
    assert_eq!(hash_of(&pool, &w.friend.id).await, before);

    // Bodies that are not exactly `{password}` get the fixed answer, whatever they quote.
    let bad: [(&str, u16, &str); 8] = [
        (r#"{"password":123}"#, 400, "invalid_request"),
        (r#"{"password":null}"#, 400, "invalid_request"),
        ("{}", 400, "invalid_request"),
        (
            r#"{"password":"correct horse battery","admin":true}"#,
            400,
            "invalid_request",
        ),
        (
            r#"{"username":"Friend_1","password":"correct horse battery"}"#,
            400,
            "invalid_request",
        ),
        ("[1,2]", 400, "invalid_request"),
        ("", 400, "invalid_request"),
        ("not json", 400, "invalid_request"),
    ];
    for (body, status, code) in bad {
        let reply = w.admin.post(addr, &path, body).await;
        assert_eq!(error_of(&reply), (status, code.into()), "{body}");
        assert_eq!(reply.json()["message"], json!(FIXED_400), "{body}");
    }
    let wrong_type = raw(
        addr,
        "POST",
        &path,
        "{}",
        "text/plain",
        Some(&w.admin.cookie),
    )
    .await;
    assert_eq!(
        error_of(&wrong_type),
        (415, "unsupported_media_type".into())
    );
    assert_eq!(hash_of(&pool, &w.friend.id).await, before);

    // Ten and 128 characters are both fine.
    for password in ["0123456789".to_owned(), "p".repeat(128)] {
        let reply = set_password(&w.admin, &w.server, &w.friend.id, &password).await;
        assert_eq!(reply.status, 204, "{}", reply.text());
        login(addr, "Friend_1", &password).await;
    }
}

#[sqlx::test(migrations = false)]
async fn making_resetting_and_removing_share_the_admins_budget_and_refusals_cost_nothing(
    pool: PgPool,
) {
    let w = world(&pool, |config| {
        config.auth.limits = AttemptLimits {
            admin_creates_per_account: 3,
            ..roomy_limits()
        };
    })
    .await;
    let addr = w.server.addr;
    // Refusals before the work cost nothing: rules, unknown ids and the admin's own account.
    for _ in 0..5 {
        let short = set_password(&w.admin, &w.server, &w.friend.id, "short").await;
        assert_eq!(short.status, 400);
        let unknown = remove(&w.admin, &w.server, &Uuid::new_v4().to_string()).await;
        assert_eq!(unknown.status, 404);
        assert_eq!(remove(&w.admin, &w.server, &w.admin.id).await.status, 409);
        let own = set_password(&w.admin, &w.server, &w.admin.id, PASSWORD).await;
        assert_eq!(own.status, 409);
    }
    assert_eq!(
        make(&w.admin, &w.server, "Made_1", PASSWORD).await.status,
        201
    );
    let reset = set_password(&w.admin, &w.server, &w.friend.id, "a fresh passphrase").await;
    assert_eq!(reset.status, 204);
    assert_eq!(remove(&w.admin, &w.server, &w.other.id).await.status, 204);

    let hash = hash_of(&pool, &w.friend.id).await;
    let made = login(addr, "Made_1", PASSWORD).await;
    let blocked = set_password(&w.admin, &w.server, &w.friend.id, "yet another phrase").await;
    assert_eq!(error_of(&blocked), (429, "too_many_attempts".into()));
    let wait: u64 = blocked.header("retry-after").unwrap().parse().unwrap();
    assert!((1..=600).contains(&wait), "{wait}");
    assert_eq!(
        hash_of(&pool, &w.friend.id).await,
        hash,
        "a blocked try changes nothing"
    );

    let blocked = remove(&w.admin, &w.server, &made.id).await;
    assert_eq!(error_of(&blocked), (429, "too_many_attempts".into()));
    assert_eq!(
        count(&pool, "users").await,
        3,
        "a blocked removal removes nothing"
    );
    let blocked = make(&w.admin, &w.server, "Made_2", PASSWORD).await;
    assert_eq!(blocked.status, 429);
    // The list is not throttled by it.
    assert_eq!(w.admin.get(addr, "/api/admin/users").await.status, 200);
}

#[sqlx::test(migrations = false)]
async fn removing_an_account_is_the_deletion_of_the_account(pool: PgPool) {
    let w = world(&pool, |_| {}).await;
    let addr = w.server.addr;
    let phone = login(addr, "Friend_1", PASSWORD).await;
    let database = Database::from_pool(pool.clone());
    for (owner, names) in [
        (&w.friend, &["One", "Two"][..]),
        (&w.other, &["Other's"][..]),
        (&w.admin, &["Admin's"][..]),
    ] {
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
    // The friend hosts a lobby with a socket open, and took a seat in the other account's lobby.
    let hosted = make_as(&w.server, &w.friend, 3).await;
    let others = make_as(&w.server, &w.other, 2).await;
    let joined = join(
        addr,
        &others.id,
        &json!({}),
        &[("Cookie", &w.friend.cookie)],
    )
    .await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    assert_eq!(seat_row(&pool, &others.id, 1).await.0, "claimed");
    let mut socket = net::open(
        addr,
        &format!("/ws/{}", hosted.id),
        &[("Cookie", &w.friend.cookie)],
    )
    .await;
    assert_eq!(socket.welcome(&hosted.id).await.index(), 0);

    let kept = mentions(&pool, &w.other.id).await;
    let before = mentions(&pool, &w.friend.id).await;
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

    let removed = remove(&w.admin, &w.server, &w.friend.id).await;
    assert_eq!(removed.status, 204, "{}", removed.text());
    assert_eq!(removed.body.len(), 0);
    // The admin's own cookie is not touched.
    sets_no_cookie(&removed);
    assert_eq!(removed.header("cache-control"), Some("no-store"));

    assert_eq!(mentions(&pool, &w.friend.id).await, Vec::<String>::new());
    assert_eq!(count(&pool, "users").await, 2);
    assert_eq!(count(&pool, "decks").await, 2);
    assert_eq!(mentions(&pool, &w.other.id).await, kept);
    for session in [&w.friend, &phone] {
        let me = session.get(addr, "/api/auth/me").await;
        assert_eq!(error_of(&me), (401, "unauthenticated".into()));
    }
    assert_eq!(w.admin.get(addr, "/api/auth/me").await.status, 200);
    assert_eq!(w.other.get(addr, "/api/auth/me").await.status, 200);

    // The seat in the other lobby is open again, and the lobby the account hosted is closed.
    assert_eq!(
        seat_row(&pool, &others.id, 1).await,
        ("open".into(), String::new(), None, false)
    );
    assert_eq!(status_of(&pool, &hosted.id).await, "expired");
    assert_eq!(socket.closed().await, Some(4002));
    let listed = lobby::listed(&lobby::lobbies(addr, &[]).await);
    assert!(
        listed.contains(&others.id) && !listed.contains(&hosted.id),
        "{listed:?}"
    );

    // A second removal finds nothing, the name is free again, and the new account holds nothing.
    let again = remove(&w.admin, &w.server, &w.friend.id).await;
    assert_eq!(error_of(&again), (404, "unknown_account".into()));
    let remade = make(&w.admin, &w.server, "Friend_1", PASSWORD).await;
    assert_eq!(remade.status, 201, "{}", remade.text());
    assert_ne!(remade.json()["id"], json!(w.friend.id));
    assert_eq!(count(&pool, "decks").await, 2);
}

#[sqlx::test(migrations = false)]
async fn a_foreign_site_cannot_set_passwords_or_remove_accounts_with_the_admins_cookie(
    pool: PgPool,
) {
    let w = world(&pool, |_| {}).await;
    let hash = hash_of(&pool, &w.friend.id).await;
    let evil = [("Origin", "https://evil.example")];
    let body = json!({ "password": "an attacker's phrase" }).to_string();
    let reset = w
        .admin
        .send(
            w.server.addr,
            "POST",
            &password_path(&w.friend.id),
            &body,
            &evil,
        )
        .await;
    assert_eq!(error_of(&reset), (403, "forbidden_origin".into()));
    let gone = w
        .admin
        .send(
            w.server.addr,
            "DELETE",
            &remove_path(&w.friend.id),
            "",
            &evil,
        )
        .await;
    assert_eq!(error_of(&gone), (403, "forbidden_origin".into()));
    assert_eq!(hash_of(&pool, &w.friend.id).await, hash);
    assert_eq!(count(&pool, "users").await, 3);
}

#[sqlx::test(migrations = false)]
async fn the_logs_name_ids_only(pool: PgPool) {
    let logs = common::capture_logs();
    let w = world(&pool, |_| {}).await;
    let fresh = format!("pw-{}", Uuid::new_v4());
    let refused = format!("wrong-{}", Uuid::new_v4());
    // A name no other test shares, since the buffer holds the whole process's logs.
    let name = format!("Log_{}", &Uuid::new_v4().simple().to_string()[..12]);
    assert_eq!(make(&w.admin, &w.server, &name, PASSWORD).await.status, 201);
    let victim = login(w.server.addr, &name, PASSWORD).await;
    assert_eq!(
        set_password(&w.admin, &w.server, &victim.id, &fresh)
            .await
            .status,
        204
    );
    assert_eq!(
        set_password(&w.admin, &w.server, &victim.id, "short")
            .await
            .status,
        400
    );
    assert_eq!(
        set_password(&w.friend, &w.server, &victim.id, &refused)
            .await
            .status,
        403
    );
    assert_eq!(remove(&w.admin, &w.server, &victim.id).await.status, 204);

    let logged = String::from_utf8_lossy(&logs.lock().unwrap()).into_owned();
    assert!(logged.contains("password set by the admin"));
    assert!(logged.contains("account removed by the admin"));
    assert!(logged.contains(&victim.id));
    for secret in [
        fresh.as_str(),
        refused.as_str(),
        name.as_str(),
        victim.token.as_str(),
        w.admin.token.as_str(),
        w.friend.token.as_str(),
    ] {
        assert!(!logged.contains(secret), "{secret} reached the logs");
    }
}

// --- the admin's own account ------------------------------------------------------------------

#[sqlx::test(migrations = false)]
async fn the_admin_cannot_delete_its_own_account_and_it_costs_no_password_try(pool: PgPool) {
    let w = world(&pool, |config| {
        config.auth.limits = AttemptLimits {
            login_per_username: 2,
            login_per_ip: 2,
            ..roomy_limits()
        };
    })
    .await;
    let addr = w.server.addr;
    let draft = DeckDraft {
        name: "Admin's".to_owned(),
        format: "commander".to_owned(),
        notes: String::new(),
        lines: vec![],
        check: None,
    };
    let owner = Uuid::parse_str(&w.admin.id).unwrap();
    Database::from_pool(pool.clone())
        .decks()
        .create(owner, &draft, 100)
        .await
        .unwrap();

    // Decided before the body is read: the right password, a wrong one and no body get the same 409,
    // and none counts as a password try, so the admin can still sign in afterwards.
    for body in [
        json!({ "password": PASSWORD }).to_string(),
        json!({ "password": "wrong wrong wrong" }).to_string(),
        "not json".to_owned(),
        String::new(),
    ] {
        for _ in 0..3 {
            let reply = w
                .admin
                .send(addr, "DELETE", "/api/auth/account", &body, &[])
                .await;
            assert_eq!(error_of(&reply), (409, "admin_account".into()), "{body}");
            assert_eq!(
                reply.json()["message"],
                json!("this account makes accounts for your friends, so it cannot be deleted")
            );
            sets_no_cookie(&reply);
        }
    }
    assert_eq!(w.admin.get(addr, "/api/auth/me").await.status, 200);
    login(addr, ADMIN, PASSWORD).await;
    assert_eq!(count(&pool, "users").await, 3);
    assert_eq!(count(&pool, "decks").await, 1);
    assert_eq!(admins(&pool).await, [ADMIN]);

    // Without a session the answer is still 401, and a friend deletes as before.
    let nobody = raw(
        addr,
        "DELETE",
        "/api/auth/account",
        "{}",
        "application/json",
        None,
    )
    .await;
    assert_eq!(error_of(&nobody), (401, "unauthenticated".into()));
    let body = json!({ "password": PASSWORD }).to_string();
    let gone = w
        .friend
        .send(addr, "DELETE", "/api/auth/account", &body, &[])
        .await;
    assert_eq!(gone.status, 204, "{}", gone.text());

    // The flag is read on every request: once the account is not the admin, it may go.
    sqlx::query("UPDATE users SET is_admin = false")
        .execute(&pool)
        .await
        .unwrap();
    let gone = w
        .admin
        .send(addr, "DELETE", "/api/auth/account", &body, &[])
        .await;
    assert_eq!(gone.status, 204, "{}", gone.text());
    assert_eq!(count(&pool, "users").await, 1);
}

// --- bodies that do not parse -------------------------------------------------------------------

#[sqlx::test(migrations = false)]
async fn every_credential_route_answers_a_body_that_does_not_parse_with_one_fixed_message(
    pool: PgPool,
) {
    let w = world(&pool, |_| {}).await;
    let addr = w.server.addr;
    let secret = "secret-value-9";
    // (method, path, who sends it); the admin's own deletion is refused before its body is read.
    let routes: Vec<(&str, String, Option<&Browser>)> = vec![
        ("POST", "/api/auth/register".into(), None),
        ("POST", "/api/auth/login".into(), None),
        ("POST", "/api/admin/users".into(), Some(&w.admin)),
        ("POST", password_path(&w.friend.id), Some(&w.admin)),
        ("POST", "/api/auth/password".into(), Some(&w.friend)),
        ("DELETE", "/api/auth/account".into(), Some(&w.friend)),
    ];
    let bodies = [
        r#"{"username":"x","password":123}"#.to_owned(),
        r#"{"password":1234567890,"username":"abc"}"#.to_owned(),
        format!(r#"{{"username":"abc","password":"{secret}","extra":true}}"#),
        format!(r#"{{"current_password":"{secret}","new_password":null}}"#),
        format!(r#"{{"password":"{secret}","keep_decks":true}}"#),
        r#"{"current_password":null}"#.to_owned(),
        "[1,2,3]".to_owned(),
        "\"just a string\"".to_owned(),
        "not json {".to_owned(),
        String::new(),
    ];
    for (method, path, who) in &routes {
        for body in &bodies {
            let reply = match who {
                Some(who) => who.send(addr, method, path, body, &[]).await,
                None => raw(addr, method, path, body, "application/json", None).await,
            };
            assert_eq!(
                error_of(&reply),
                (400, "invalid_request".into()),
                "{method} {path} {body}"
            );
            let message = reply.json()["message"].as_str().unwrap().to_owned();
            assert_eq!(message, FIXED_400, "{method} {path} {body}");
            assert!(!reply.text().contains(secret) && !reply.text().contains("123"));
        }
    }
    // The other routes keep naming what is missing; only the ones with a password stay silent.
    let join = raw(addr, "POST", "/api/join", "{}", "application/json", None).await;
    assert_eq!(join.status, 400);
    let message: Value = join.json()["message"].clone();
    assert!(
        message.as_str().unwrap().contains("missing field"),
        "{message}"
    );
}
