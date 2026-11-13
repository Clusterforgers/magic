//! Goal: the art endpoints work over real HTTP on Postgres: a card's printings list newest first with their look and need no account, a deck line's art is set and cleared only by the deck's owner and only to a printing of the card, the preferred art of an account is kept per card and shows in its deck list, and a malformed request is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::sync::OnceLock;

use mtg_cards::Caps;
use mtg_db::Database;
use mtg_import::scryfall::cards;
use mtg_server::{CardSource, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, get, register};
use serde_json::{Value, json};
use sqlx::PgPool;

fn objects() -> &'static Vec<Value> {
    static OBJECTS: OnceLock<Vec<Value>> = OnceLock::new();
    OBJECTS.get_or_init(|| {
        serde_json::from_str(&mtg_testkit::corpus::scryfall_array().unwrap()).unwrap()
    })
}

fn object(name: &str) -> &'static Value {
    objects()
        .iter()
        .find(|card| card["name"] == name)
        .unwrap_or_else(|| panic!("the corpus has no {name}"))
}

fn oracle(name: &str) -> String {
    object(name)["oracle_id"].as_str().unwrap().to_owned()
}

fn printing(name: &str) -> String {
    object(name)["id"].as_str().unwrap().to_owned()
}

/// A second printing of Forest, newer than the corpus one, full art in two finishes.
const NEW_FOREST: &str = "00000000-0000-4000-8000-00000000f0e5";

fn new_forest() -> Value {
    let mut forest = object("Forest").clone();
    forest["id"] = json!(NEW_FOREST);
    forest["set"] = json!("zzz");
    forest["set_name"] = json!("Zenith Lands");
    forest["collector_number"] = json!("300");
    forest["released_at"] = json!("2099-01-01");
    forest["frame"] = json!("2015");
    forest["finishes"] = json!(["nonfoil", "foil"]);
    forest["full_art"] = json!(true);
    forest
}

async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    let mut all = objects().clone();
    for card in &mut all {
        card["legalities"] = json!({"modern": "legal"});
    }
    let mut all_json = serde_json::to_string(&all).unwrap();
    db.importer()
        .import_cards(cards(all_json.as_bytes()))
        .await
        .unwrap();
    all_json = serde_json::to_string(&[new_forest()]).unwrap();
    db.importer()
        .import_printings(cards(all_json.as_bytes()))
        .await
        .unwrap();
    let source = CardSource::new(pool.clone()).with_caps(Caps::everything());
    TestServer::start_with_cards(pool.clone(), bot_config(), source).await
}

/// Ann's deck of Forests and Lightning Bolts; its id.
async fn deck_of(server: &TestServer, who: &Browser) -> String {
    let body = json!({"name": "Lands", "format": "freeform", "notes": "", "cards": [
        {"zone": "main", "oracle_id": oracle("Forest"), "printing_id": null, "count": 20},
        {"zone": "sideboard", "oracle_id": oracle("Forest"), "printing_id": null, "count": 1},
        {"zone": "main", "oracle_id": oracle("Lightning Bolt"), "printing_id": null, "count": 4},
    ]});
    let reply = who.post(server.addr, "/api/decks", &body.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

fn choice(zone: &str, card: &str, printing: Option<&str>) -> String {
    json!({"zone": zone, "oracle_id": oracle(card), "printing_id": printing}).to_string()
}

/// The printing of each line of the deck, in order.
async fn art_of(server: &TestServer, who: &Browser, deck: &str) -> Vec<Value> {
    let view = who
        .get(server.addr, &format!("/api/decks/{deck}"))
        .await
        .json();
    view["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line["printing_id"].clone())
        .collect()
}

#[sqlx::test(migrations = false)]
async fn a_cards_printings_list_newest_first_with_their_look(pool: PgPool) {
    let server = start(&pool).await;
    let reply = get(
        server.addr,
        &format!("/api/cards/{}/printings", oracle("Forest").to_uppercase()),
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let printings = reply.json()["printings"].as_array().unwrap().clone();
    let ids: Vec<&str> = printings
        .iter()
        .map(|p| p["printing_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [NEW_FOREST, printing("Forest").as_str()]);
    let newest = &printings[0];
    assert_eq!(newest["set_name"], "Zenith Lands");
    assert_eq!(newest["frame"], "2015");
    assert_eq!(newest["finishes"], json!(["nonfoil", "foil"]));
    assert_eq!(newest["full_art"], true);
    assert!(newest["images"].is_object());

    for path in [
        "/api/cards/not-an-id/printings",
        "/api/cards/00000000-0000-4000-8000-000000000001/printings",
    ] {
        assert_eq!(get(server.addr, path).await.status, 404, "{path}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_lines_art_is_its_owners_and_a_printing_of_its_card(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;
    let deck = deck_of(&server, &ann).await;
    let path = format!("/api/decks/{deck}/printing");
    let other = "00000000-0000-4000-8000-0000000000ff";

    let refused = [
        (
            "another card's printing",
            &ann,
            path.clone(),
            choice("main", "Lightning Bolt", Some(NEW_FOREST)),
            400,
            "invalid_deck",
        ),
        (
            "a printing nobody prints",
            &ann,
            path.clone(),
            choice("main", "Forest", Some(other)),
            400,
            "invalid_deck",
        ),
        (
            "a printing that is no id",
            &ann,
            path.clone(),
            choice("main", "Forest", Some("x")),
            400,
            "invalid_request",
        ),
        (
            "a zone without the card",
            &ann,
            path.clone(),
            choice("commander", "Forest", Some(NEW_FOREST)),
            404,
            "not_found",
        ),
        (
            "another account's deck",
            &bob,
            path.clone(),
            choice("main", "Forest", Some(NEW_FOREST)),
            404,
            "not_found",
        ),
        (
            "a deck nobody has",
            &ann,
            format!("/api/decks/{other}/printing"),
            choice("main", "Forest", None),
            404,
            "not_found",
        ),
        (
            "a body of other fields",
            &ann,
            path.clone(),
            json!({"zone": "main", "oracle_id": oracle("Forest"), "printing_id": null, "count": 2})
                .to_string(),
            400,
            "invalid_request",
        ),
    ];
    for (case, who, at, body, status, code) in refused {
        let reply = who.put(server.addr, &at, &body).await;
        assert_eq!(reply.status, status, "{case}: {}", reply.text());
        assert_eq!(reply.json()["error"], code, "{case}");
    }
    assert_eq!(
        art_of(&server, &ann, &deck).await,
        [Value::Null, Value::Null, Value::Null],
        "nothing changed"
    );

    let reply = net::request(
        server.addr,
        "PUT",
        &path,
        &[("Content-Type", "application/json")],
        choice("main", "Forest", None).as_bytes(),
    )
    .await;
    assert_eq!(reply.status, 401, "a request without an account");

    let reply = ann
        .put(
            server.addr,
            &path,
            &choice("main", "Forest", Some(&NEW_FOREST.to_uppercase())),
        )
        .await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    assert_eq!(
        art_of(&server, &ann, &deck).await,
        [json!(NEW_FOREST), Value::Null, Value::Null],
        "only the main line"
    );

    let reply = ann
        .put(server.addr, &path, &choice("main", "Forest", None))
        .await;
    assert_eq!(reply.status, 204);
    assert_eq!(
        art_of(&server, &ann, &deck).await,
        [Value::Null, Value::Null, Value::Null]
    );
}

#[sqlx::test(migrations = false)]
async fn the_preferred_art_is_kept_per_card_and_shows_in_the_deck_list(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;
    deck_of(&server, &ann).await;
    let forest = format!("/api/me/printings/{}", oracle("Forest"));
    let pick = |id: &str| json!({"printing_id": id}).to_string();

    let reply = ann
        .put(
            server.addr,
            &format!("/api/me/printings/{}", oracle("Lightning Bolt")),
            &pick(NEW_FOREST),
        )
        .await;
    assert_eq!(
        (reply.status, reply.json()["error"].clone()),
        (400, json!("invalid_deck"))
    );
    assert_eq!(
        ann.put(server.addr, &forest, &pick(NEW_FOREST))
            .await
            .status,
        204
    );

    let mine = ann.get(server.addr, "/api/me/printings").await.json();
    assert_eq!(mine, json!({"printings": {oracle("Forest"): NEW_FOREST}}));
    assert_eq!(
        bob.get(server.addr, "/api/me/printings").await.json(),
        json!({"printings": {}})
    );

    let list = ann.get(server.addr, "/api/decks").await.json();
    assert_eq!(
        list["decks"][0]["pictures"][0]["oracle_id"],
        oracle("Forest")
    );
    assert_eq!(
        list["decks"][0]["pictures"][0]["printing_id"], NEW_FOREST,
        "the list shows the preferred art"
    );

    assert_eq!(ann.delete(server.addr, &forest).await.status, 204);
    assert_eq!(
        ann.delete(server.addr, &forest).await.status,
        204,
        "forgetting twice is fine"
    );
    let list = ann.get(server.addr, "/api/decks").await.json();
    assert!(list["decks"][0]["pictures"][0].get("printing_id").is_none());
    assert_eq!(get(server.addr, "/api/me/printings").await.status, 401);
}
