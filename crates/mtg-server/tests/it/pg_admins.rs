//! Goal: several accounts are admins at once: every account `MTG_ADMIN_USERNAMES` names is marked at start, each admin makes accounts and lists them, no admin account can be deleted or given a new password by another admin or in Account settings, and an account left out of the list is a plain account from the next start on.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;
use crate::pg_admin::{admins, error_of, roomy_limits};

use mtg_db::Database;
use mtg_server::{ServerConfig, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, credentials, login, register};
use serde_json::json;
use sqlx::PgPool;

/// Both schemas migrated behind a running server whose admins are `names` (as the setting spells them).
async fn start(pool: &PgPool, names: &[&str]) -> TestServer {
    migrate(pool).await.unwrap();
    Database::from_pool(pool.clone()).migrate().await.unwrap();
    let mut config: ServerConfig = bot_config();
    config.auth.limits = roomy_limits();
    config.auth.admins = names.iter().map(|name| (*name).to_owned()).collect();
    TestServer::start_on(pool.clone(), config).await
}

/// `Chrisser`, `Ebbe` and `Friend_1` registered before a server with `names` as admins starts.
async fn world(pool: &PgPool, names: &[&str]) -> TestServer {
    let first = start(pool, &[]).await;
    for name in ["Chrisser", "Ebbe", "Friend_1"] {
        register(first.addr, name, PASSWORD).await;
    }
    drop(first);
    start(pool, names).await
}

async fn signed_in(server: &TestServer, name: &str) -> Browser {
    login(server.addr, name, PASSWORD).await
}

#[sqlx::test(migrations = false)]
async fn both_admins_make_accounts_and_see_each_other_as_admins(pool: PgPool) {
    let server = world(&pool, &["chrisser", "EBBE"]).await;
    assert_eq!(admins(&pool).await, ["Chrisser", "Ebbe"]);
    for (admin, made) in [("Chrisser", "Made_by_c"), ("Ebbe", "Made_by_e")] {
        let browser = signed_in(&server, admin).await;
        let me = browser.get(server.addr, "/api/auth/me").await;
        assert_eq!(me.json()["is_admin"], json!(true), "{admin}");
        let reply = browser
            .post(
                server.addr,
                "/api/admin/users",
                &credentials(made, PASSWORD),
            )
            .await;
        assert_eq!(reply.status, 201, "{admin}");
        let listed = browser.get(server.addr, "/api/admin/users").await;
        let flags: Vec<(String, bool)> = listed.json()["users"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let name = row["username"].as_str().unwrap().to_owned();
                (name, row["is_admin"].as_bool().unwrap())
            })
            .collect();
        assert!(flags.contains(&("Chrisser".into(), true)), "{flags:?}");
        assert!(flags.contains(&("Ebbe".into(), true)), "{flags:?}");
        assert!(flags.contains(&("Friend_1".into(), false)), "{flags:?}");
    }
    login(server.addr, "Made_by_c", PASSWORD).await;
    login(server.addr, "Made_by_e", PASSWORD).await;
    let friend = signed_in(&server, "Friend_1").await;
    let reply = friend
        .post(
            server.addr,
            "/api/admin/users",
            &credentials("Nope_1", PASSWORD),
        )
        .await;
    assert_eq!(error_of(&reply), (403, "forbidden".into()));
}

#[sqlx::test(migrations = false)]
async fn no_admin_account_can_be_deleted_or_reset_by_any_admin(pool: PgPool) {
    let server = world(&pool, &["Chrisser", "Ebbe"]).await;
    let addr = server.addr;
    let (chrisser, ebbe) = (
        signed_in(&server, "Chrisser").await,
        signed_in(&server, "Ebbe").await,
    );
    let body = json!({ "password": "a fresh password 1" }).to_string();
    for by in [&chrisser, &ebbe] {
        for target in [&chrisser, &ebbe] {
            let path = format!("/api/admin/users/{}", target.id);
            let reply = by.send(addr, "DELETE", &path, "", &[]).await;
            assert_eq!(error_of(&reply), (409, "admin_account".into()), "{path}");
            let reply = by.post(addr, &format!("{path}/password"), &body).await;
            assert_eq!(error_of(&reply), (409, "admin_account".into()), "{path}");
        }
        let own = json!({ "password": PASSWORD }).to_string();
        let reply = by
            .send(addr, "DELETE", "/api/auth/account", &own, &[])
            .await;
        assert_eq!(error_of(&reply), (409, "admin_account".into()));
    }
    assert_eq!(admins(&pool).await, ["Chrisser", "Ebbe"]);
    // Both keep working afterwards.
    assert_eq!(chrisser.get(addr, "/api/auth/me").await.status, 200);
    assert_eq!(ebbe.get(addr, "/api/auth/me").await.status, 200);
}

#[sqlx::test(migrations = false)]
async fn an_account_left_out_of_the_list_is_demoted_at_the_next_start(pool: PgPool) {
    let server = world(&pool, &["Chrisser", "Ebbe"]).await;
    assert_eq!(admins(&pool).await, ["Chrisser", "Ebbe"]);
    drop(server);

    let server = start(&pool, &["Chrisser"]).await;
    assert_eq!(admins(&pool).await, ["Chrisser"]);
    let ebbe = signed_in(&server, "Ebbe").await;
    let me = ebbe.get(server.addr, "/api/auth/me").await;
    assert_eq!(me.json()["is_admin"], json!(false));
    let reply = ebbe
        .post(
            server.addr,
            "/api/admin/users",
            &credentials("Nope_1", PASSWORD),
        )
        .await;
    assert_eq!(error_of(&reply), (403, "forbidden".into()));
    // A plain account again: the remaining admin can remove it.
    let chrisser = signed_in(&server, "Chrisser").await;
    let path = format!("/api/admin/users/{}", ebbe.id);
    let reply = chrisser.send(server.addr, "DELETE", &path, "", &[]).await;
    assert_eq!(reply.status, 204);
    assert_eq!(admins(&pool).await, ["Chrisser"]);
}
