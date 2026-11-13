//! Goal: deck groups work end to end over real HTTP on Postgres: every route needs a signed-in account, groups are created in order, renamed, collapsed, reordered and deleted, a deck sits in one group at most and moves between them, deleting a group leaves its decks ungrouped, another account's groups and decks are plain 404s, the count is capped, and deleting an account removes its groups.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_cards::Caps;
use mtg_db::Database;
use mtg_import::scryfall::cards;
use mtg_protocol::rest::MAX_DECK_GROUPS_PER_USER;
use mtg_server::{CardSource, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

const ROUTE: &str = "/api/deck-groups";
const NO_SUCH: &str = "00000000-0000-4000-8000-000000000000";

async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    let database = Database::from_pool(pool.clone());
    database.migrate().await.unwrap();
    database
        .importer()
        .import_cards(cards(
            mtg_testkit::corpus::scryfall_array().unwrap().as_bytes(),
        ))
        .await
        .unwrap();
    TestServer::start_with_cards(
        pool.clone(),
        bot_config(),
        CardSource::new(pool.clone()).with_caps(Caps::everything()),
    )
    .await
}

fn forest() -> String {
    let all: Vec<Value> =
        serde_json::from_str(&mtg_testkit::corpus::scryfall_array().unwrap()).unwrap();
    all.iter().find(|card| card["name"] == "Forest").unwrap()["oracle_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn deck(server: &TestServer, who: &Browser, name: &str) -> String {
    let body = json!({"name": name, "format": "freeform", "notes": "",
        "cards": [{"zone": "main", "oracle_id": forest(), "printing_id": null, "count": 1}]});
    let reply = who.post(server.addr, "/api/decks", &body.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

async fn group(server: &TestServer, who: &Browser, name: &str) -> String {
    let reply = who
        .post(server.addr, ROUTE, &json!({ "name": name }).to_string())
        .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    assert_eq!(reply.json()["collapsed"], json!(false));
    reply.json()["id"].as_str().unwrap().to_owned()
}

async fn groups(server: &TestServer, who: &Browser) -> Vec<Value> {
    let reply = who.get(server.addr, ROUTE).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["groups"].as_array().unwrap().clone()
}

/// The groups as `(name, collapsed, decks)`.
async fn shelf(server: &TestServer, who: &Browser) -> Vec<(String, bool, Vec<String>)> {
    groups(server, who)
        .await
        .into_iter()
        .map(|g| {
            (
                g["name"].as_str().unwrap().to_owned(),
                g["collapsed"].as_bool().unwrap(),
                g["decks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d.as_str().unwrap().to_owned())
                    .collect(),
            )
        })
        .collect()
}

async fn file(server: &TestServer, who: &Browser, deck: &str, group: Option<&str>) -> net::Reply {
    who.put(
        server.addr,
        &format!("/api/decks/{deck}/group"),
        &json!({ "group": group }).to_string(),
    )
    .await
}

async fn patch(server: &TestServer, who: &Browser, id: &str, body: Value) -> net::Reply {
    who.send(
        server.addr,
        "PATCH",
        &format!("{ROUTE}/{id}"),
        &body.to_string(),
        &[],
    )
    .await
}

fn routes() -> Vec<(&'static str, String)> {
    vec![
        ("GET", ROUTE.to_owned()),
        ("POST", ROUTE.to_owned()),
        ("PUT", format!("{ROUTE}/order")),
        ("PATCH", format!("{ROUTE}/{NO_SUCH}")),
        ("DELETE", format!("{ROUTE}/{NO_SUCH}")),
        ("PUT", format!("/api/decks/{NO_SUCH}/group")),
    ]
}

#[tokio::test]
async fn a_server_without_a_database_has_no_groups() {
    let server = TestServer::start(bot_config()).await;
    for (method, route) in routes() {
        let reply = request(server.addr, method, &route, &[], b"").await;
        assert_eq!(reply.status, 503, "{method} {route}");
        assert_eq!(reply.error_code(), "decks_unavailable", "{method} {route}");
    }
}

#[sqlx::test(migrations = false)]
async fn every_group_route_needs_a_signed_in_account(pool: PgPool) {
    let server = start(&pool).await;
    for (method, route) in routes() {
        let reply = request(server.addr, method, &route, &[], b"{}").await;
        assert_eq!(reply.status, 401, "{method} {route}");
        assert_eq!(reply.error_code(), "unauthenticated", "{method} {route}");
    }
}

#[sqlx::test(migrations = false)]
async fn groups_are_created_in_order_renamed_collapsed_and_reordered(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    assert!(groups(&server, &ann).await.is_empty());
    let fun = group(&server, &ann, " Fun ").await;
    let hard = group(&server, &ann, "Hardcore").await;
    let dungeon = group(&server, &ann, "Dungeon").await;
    let names =
        |s: Vec<(String, bool, Vec<String>)>| s.into_iter().map(|g| g.0).collect::<Vec<_>>();
    assert_eq!(
        names(shelf(&server, &ann).await),
        ["Fun", "Hardcore", "Dungeon"]
    );

    let renamed = patch(&server, &ann, &hard, json!({"name": "Spike"})).await;
    assert_eq!(renamed.status, 200, "{}", renamed.text());
    assert_eq!(renamed.json()["name"], "Spike");
    let folded = patch(&server, &ann, &fun, json!({"collapsed": true})).await;
    assert_eq!(folded.json()["collapsed"], json!(true));
    assert_eq!(folded.json()["name"], "Fun");

    let order = json!({"ids": [dungeon, fun, hard]}).to_string();
    let reply = ann
        .put(server.addr, &format!("{ROUTE}/order"), &order)
        .await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    assert_eq!(
        shelf(&server, &ann).await,
        [
            ("Dungeon".to_owned(), false, vec![]),
            ("Fun".to_owned(), true, vec![]),
            ("Spike".to_owned(), false, vec![]),
        ]
    );
    // A new group goes last after a reorder too.
    group(&server, &ann, "Casual").await;
    assert_eq!(names(shelf(&server, &ann).await).last().unwrap(), "Casual");
}

#[sqlx::test(migrations = false)]
async fn bad_names_and_orders_are_400(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let fun = group(&server, &ann, "Fun").await;
    let hard = group(&server, &ann, "Hard").await;
    for name in ["", "   ", &"x".repeat(61)] {
        let body = json!({ "name": name }).to_string();
        let reply = ann.post(server.addr, ROUTE, &body).await;
        assert_eq!(reply.status, 400, "{name:?}");
        assert_eq!(
            patch(&server, &ann, &fun, json!({ "name": name }))
                .await
                .status,
            400
        );
    }
    for ids in [
        json!([fun]),
        json!([fun, fun]),
        json!([fun, hard, NO_SUCH]),
        json!(["not an id", hard]),
    ] {
        let body = json!({ "ids": ids }).to_string();
        let reply = ann.put(server.addr, &format!("{ROUTE}/order"), &body).await;
        assert_eq!(reply.status, 400, "{ids}");
        assert_eq!(reply.error_code(), "invalid_request");
    }
}

#[sqlx::test(migrations = false)]
async fn a_deck_sits_in_one_group_and_deleting_the_group_ungroups_it(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let (one, two) = (
        deck(&server, &ann, "One").await,
        deck(&server, &ann, "Two").await,
    );
    let fun = group(&server, &ann, "Fun").await;
    let hard = group(&server, &ann, "Hardcore").await;

    for (d, g) in [(&one, &fun), (&two, &fun), (&one, &hard)] {
        assert_eq!(file(&server, &ann, d, Some(g)).await.status, 204);
    }
    assert_eq!(
        shelf(&server, &ann).await,
        [
            ("Fun".to_owned(), false, vec![two.clone()]),
            ("Hardcore".to_owned(), false, vec![one.clone()]),
        ]
    );
    assert_eq!(file(&server, &ann, &one, None).await.status, 204);
    assert_eq!(file(&server, &ann, &one, None).await.status, 204);
    assert!(shelf(&server, &ann).await[1].2.is_empty());

    assert_eq!(
        ann.delete(server.addr, &format!("{ROUTE}/{fun}"))
            .await
            .status,
        204
    );
    assert_eq!(
        ann.delete(server.addr, &format!("{ROUTE}/{fun}"))
            .await
            .status,
        404
    );
    assert_eq!(shelf(&server, &ann).await.len(), 1);
    // The decks stay.
    let decks = ann.get(server.addr, "/api/decks").await.json();
    assert_eq!(decks["decks"].as_array().unwrap().len(), 2);
    // Deleting a deck takes it out of its group.
    assert_eq!(file(&server, &ann, &two, Some(&hard)).await.status, 204);
    assert_eq!(
        ann.delete(server.addr, &format!("/api/decks/{two}"))
            .await
            .status,
        204
    );
    assert!(shelf(&server, &ann).await[0].2.is_empty());
}

#[sqlx::test(migrations = false)]
async fn another_accounts_groups_and_decks_are_404(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let anns_deck = deck(&server, &ann, "Ann's").await;
    let anns_group = group(&server, &ann, "Fun").await;
    let bos_deck = deck(&server, &bo, "Bob's").await;
    let bos_group = group(&server, &bo, "Bob's").await;

    assert!(shelf(&server, &bo).await.iter().all(|g| g.0 == "Bob's"));
    for reply in [
        file(&server, &bo, &anns_deck, Some(&bos_group)).await,
        file(&server, &bo, &bos_deck, Some(&anns_group)).await,
        file(&server, &bo, &bos_deck, Some("not an id")).await,
        file(&server, &bo, &anns_deck, None).await,
        patch(&server, &bo, &anns_group, json!({"name": "Mine"})).await,
        bo.delete(server.addr, &format!("{ROUTE}/{anns_group}"))
            .await,
    ] {
        assert_eq!(reply.status, 404, "{}", reply.text());
        assert_eq!(reply.error_code(), "not_found");
    }
    let order = json!({ "ids": [bos_group, anns_group] }).to_string();
    assert_eq!(
        bo.put(server.addr, &format!("{ROUTE}/order"), &order)
            .await
            .status,
        400
    );
    assert_eq!(
        shelf(&server, &ann).await,
        [("Fun".to_owned(), false, vec![])]
    );
}

#[sqlx::test(migrations = false)]
async fn the_count_of_groups_is_capped(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    for n in 0..MAX_DECK_GROUPS_PER_USER {
        group(&server, &ann, &format!("G{n}")).await;
    }
    let reply = ann
        .post(server.addr, ROUTE, &json!({"name": "One more"}).to_string())
        .await;
    assert_eq!(reply.status, 409);
    assert_eq!(reply.error_code(), "group_limit");
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_removes_its_groups_and_only_its_groups(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let d = deck(&server, &ann, "One").await;
    let g = group(&server, &ann, "Fun").await;
    file(&server, &ann, &d, Some(&g)).await;
    group(&server, &bo, "Bob's").await;

    let body = json!({"password": PASSWORD}).to_string();
    let gone = ann
        .send(server.addr, "DELETE", "/api/auth/account", &body, &[])
        .await;
    assert_eq!(gone.status, 204, "{}", gone.text());
    let left: Vec<String> = sqlx::query_scalar("SELECT name FROM deck_groups ORDER BY name")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(left, ["Bob's"]);
}
