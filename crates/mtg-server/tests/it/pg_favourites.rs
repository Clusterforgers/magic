//! Goal: favourite precons work end to end over real HTTP on Postgres: only a signed-in account reaches them and only its own stars, starring is idempotent and the newest star comes first, a code nobody has is a plain 404 while unstarring never fails, a star of a precon that is gone is not listed, writes count per account against a limit with `Retry-After`, deleting an account removes its stars, a server without a database says so, and the deck lists carry the pictures that stand for each deck (its commanders, else its first card).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::time::Duration;

use mtg_cards::Caps;
use mtg_db::Database;
use mtg_import::mtgjson::{PreconCard, PreconDeck};
use mtg_import::scryfall::cards;
use mtg_server::{CardSource, ServerConfig, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

const EVERGREEN: &str = "tst-evergreen-pack";
const COMMANDER: &str = "tst-commander-pack";
const ROUTE: &str = "/api/favourites/precons";

fn oracle(name: &str) -> String {
    let all: Vec<Value> =
        serde_json::from_str(&mtg_testkit::corpus::scryfall_array().unwrap()).unwrap();
    all.iter()
        .find(|card| card["name"] == name)
        .unwrap_or_else(|| panic!("the corpus has no {name}"))["oracle_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn entry(name: &str, count: u32) -> PreconCard {
    PreconCard {
        count,
        uuid: format!("uuid-{name}"),
        name: name.to_owned(),
        set_code: None,
        number: None,
        is_foil: false,
        scryfall_id: None,
        oracle_id: None,
    }
}

fn deck(name: &str, commander: &[(&str, u32)], main: &[(&str, u32)]) -> PreconDeck {
    let cards = |list: &[(&str, u32)]| list.iter().map(|(n, c)| entry(n, *c)).collect();
    PreconDeck {
        name: name.to_owned(),
        code: "TST".to_owned(),
        deck_type: Some("Starter Deck".to_owned()),
        release_date: Some("2024-05-01".to_owned()),
        main_board: cards(main),
        side_board: Vec::new(),
        commander: cards(commander),
        display_commander: Vec::new(),
    }
}

async fn start_with(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> TestServer {
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
    let decks = vec![
        deck("Evergreen Pack", &[], &[("Forest", 60)]),
        deck(
            "Commander Pack",
            &[("Questing Beast", 1)],
            &[("Forest", 98), ("Auspicious Starrix", 1)],
        ),
    ];
    database
        .importer()
        .import_precons(decks.into_iter().map(Ok))
        .await
        .unwrap();
    let mut config = bot_config();
    change(&mut config);
    TestServer::start_with_cards(
        pool.clone(),
        config,
        CardSource::new(pool.clone()).with_caps(Caps::everything()),
    )
    .await
}

async fn start(pool: &PgPool) -> TestServer {
    start_with(pool, |_| {}).await
}

fn path(code: &str) -> String {
    format!("{ROUTE}/{code}")
}

async fn star(server: &TestServer, who: &Browser, code: &str) -> net::Reply {
    who.put(server.addr, &path(code), "").await
}

async fn stars(server: &TestServer, who: &Browser) -> Vec<String> {
    let reply = who.get(server.addr, ROUTE).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code.as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_server_without_a_database_has_no_favourites() {
    let server = TestServer::start(bot_config()).await;
    for (method, route) in [
        ("GET", ROUTE.to_owned()),
        ("PUT", path(EVERGREEN)),
        ("DELETE", path(EVERGREEN)),
    ] {
        let reply = request(server.addr, method, &route, &[], b"").await;
        assert_eq!(reply.status, 503, "{method} {route}");
        assert_eq!(reply.error_code(), "cards_unavailable", "{method} {route}");
    }
}

#[sqlx::test(migrations = false)]
async fn every_favourite_route_needs_a_signed_in_account(pool: PgPool) {
    let server = start(&pool).await;
    for (method, route) in [
        ("GET", ROUTE.to_owned()),
        ("PUT", path(EVERGREEN)),
        ("DELETE", path(EVERGREEN)),
    ] {
        let reply = request(server.addr, method, &route, &[], b"").await;
        assert_eq!(reply.status, 401, "{method} {route}");
        assert_eq!(reply.error_code(), "unauthenticated", "{method} {route}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_star_is_idempotent_listed_newest_first_and_private_to_its_account(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    assert_eq!(stars(&server, &ann).await, Vec::<String>::new());

    for code in [EVERGREEN, COMMANDER, EVERGREEN] {
        assert_eq!(star(&server, &ann, code).await.status, 204, "{code}");
    }
    assert_eq!(stars(&server, &ann).await, [COMMANDER, EVERGREEN]);
    assert_eq!(stars(&server, &bo).await, Vec::<String>::new());

    assert_eq!(ann.delete(server.addr, &path(EVERGREEN)).await.status, 204);
    assert_eq!(ann.delete(server.addr, &path(EVERGREEN)).await.status, 204);
    assert_eq!(stars(&server, &ann).await, [COMMANDER]);
    // Bob unstarring a precon only Ann starred changes nothing for Ann.
    assert_eq!(bo.delete(server.addr, &path(COMMANDER)).await.status, 204);
    assert_eq!(stars(&server, &ann).await, [COMMANDER]);
}

#[sqlx::test(migrations = false)]
async fn a_code_nobody_has_is_404_and_a_code_in_capitals_is_the_same_precon(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    for code in ["tst-nothing-here", "Not%20A%20Code", "precon:tst"] {
        let reply = star(&server, &ann, code).await;
        assert_eq!(reply.status, 404, "{code}");
        assert_eq!(reply.error_code(), "not_found", "{code}");
    }
    assert_eq!(star(&server, &ann, "TST-EVERGREEN-PACK").await.status, 204);
    assert_eq!(stars(&server, &ann).await, [EVERGREEN]);
    // Unstarring never fails, so a star of a precon that is gone can be cleared.
    assert_eq!(
        ann.delete(server.addr, &path("tst-nothing-here"))
            .await
            .status,
        204
    );
}

#[sqlx::test(migrations = false)]
async fn a_star_of_a_precon_that_is_gone_is_not_listed(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    assert_eq!(star(&server, &ann, EVERGREEN).await.status, 204);
    sqlx::query(
        "INSERT INTO favourite_precons (user_id, code)
         SELECT id, 'tst-removed-pack' FROM users WHERE username = 'ann'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(stars(&server, &ann).await, [EVERGREEN]);
}

#[sqlx::test(migrations = false)]
async fn writes_count_per_account_and_reads_do_not(pool: PgPool) {
    let server = start_with(&pool, |config| {
        config.auth.limits.favourite_writes_per_user = 3;
        config.auth.limits.window = Duration::from_secs(2);
    })
    .await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;

    assert_eq!(star(&server, &ann, EVERGREEN).await.status, 204);
    assert_eq!(star(&server, &ann, COMMANDER).await.status, 204);
    // An unknown code counts too, so the route cannot be probed for free.
    assert_eq!(star(&server, &ann, "tst-nothing-here").await.status, 404);
    let turned_away = ann.delete(server.addr, &path(EVERGREEN)).await;
    assert_eq!(turned_away.status, 429);
    assert_eq!(turned_away.error_code(), "rate_limited");
    assert!(matches!(turned_away.header("retry-after"), Some("1" | "2")));

    // Reading is free, and another account has a budget of its own.
    assert_eq!(stars(&server, &ann).await, [COMMANDER, EVERGREEN]);
    assert_eq!(star(&server, &bo, EVERGREEN).await.status, 204);

    tokio::time::sleep(Duration::from_millis(2_300)).await;
    assert_eq!(ann.delete(server.addr, &path(EVERGREEN)).await.status, 204);
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_removes_its_stars_and_only_its_stars(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    star(&server, &ann, EVERGREEN).await;
    star(&server, &ann, COMMANDER).await;
    star(&server, &bo, EVERGREEN).await;

    let body = json!({"password": PASSWORD}).to_string();
    let gone = ann
        .send(server.addr, "DELETE", "/api/auth/account", &body, &[])
        .await;
    assert_eq!(gone.status, 204, "{}", gone.text());

    let left: Vec<String> = sqlx::query_scalar("SELECT code FROM favourite_precons ORDER BY code")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(left, [EVERGREEN]);
    assert_eq!(stars(&server, &bo).await, [EVERGREEN]);
}

#[sqlx::test(migrations = false)]
async fn the_precon_list_carries_the_commanders_pictures_else_the_first_card(pool: PgPool) {
    let server = start(&pool).await;
    let listed = net::get(server.addr, "/api/precons").await;
    assert_eq!(listed.status, 200, "{}", listed.text());
    let list = listed.json();
    let pictures = |code: &str| -> Value {
        list["precons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["code"] == code)
            .unwrap_or_else(|| panic!("no precon {code}"))["pictures"]
            .clone()
    };
    assert_eq!(
        pictures(COMMANDER),
        json!([{"oracle_id": oracle("Questing Beast"), "name": "Questing Beast"}])
    );
    assert_eq!(
        pictures(EVERGREEN),
        json!([{"oracle_id": oracle("Forest"), "name": "Forest"}])
    );
}

#[sqlx::test(migrations = false)]
async fn the_deck_list_carries_the_commanders_pictures_else_the_first_main_card(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let line = |zone: &str, name: &str, count: u32| json!({"zone": zone, "oracle_id": oracle(name), "printing_id": null, "count": count});
    let green = json!({"name": "Green", "format": "freeform", "notes": "",
        "cards": [line("main", "Giant Growth", 4), line("main", "Forest", 20)]});
    let beast = json!({"name": "Beast", "format": "commander", "notes": "",
        "cards": [line("commander", "Questing Beast", 1), line("main", "Forest", 99)]});
    for body in [&green, &beast] {
        let made = ann.post(server.addr, "/api/decks", &body.to_string()).await;
        assert_eq!(made.status, 201, "{}", made.text());
    }
    let listed = ann.get(server.addr, "/api/decks").await.json();
    let pictures = |name: &str| -> Value {
        listed["decks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == name)
            .unwrap()["pictures"]
            .clone()
    };
    assert_eq!(
        pictures("Green"),
        json!([{"oracle_id": oracle("Giant Growth"), "name": "Giant Growth"}])
    );
    assert_eq!(
        pictures("Beast"),
        json!([{"oracle_id": oracle("Questing Beast"), "name": "Questing Beast"}])
    );
}
