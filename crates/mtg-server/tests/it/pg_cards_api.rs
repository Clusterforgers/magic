//! Goal: the public card endpoints answer over real HTTP on Postgres from the recorded Scryfall fixture: free text, color, identity, type, mana value and format filters return exactly the cards they should in a stable order, pages tile the result and report its total, the page size is capped, bad queries are 400s that say why, each hit carries what a deck list needs (ids, cost, types, colors, art, support), a card's page carries its faces, legalities, printings and rulings, unknown ids are 404, and a server without a database says so.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::collections::BTreeSet;

use mtg_cards::Caps;
use mtg_db::Database;
use mtg_import::scryfall::{cards, rulings};
use mtg_server::{CardSource, migrate};
use net::{TestServer, bot_config, get, request};
use serde_json::{Value, json};
use sqlx::PgPool;

/// 24 Scryfall objects of the importer's tests: 20 deck cards, a token, an emblem and two oddities.
const CARDS: &str = include_str!("../../../mtg-import/tests/fixtures/scryfall_oracle_cards.json");
const RULINGS: &str = include_str!("../../../mtg-import/tests/fixtures/scryfall_rulings.json");

const ALL: [&str; 20] = [
    "Æther Vial",
    "Arcane Signet",
    "Atraxa, Praetors' Voice",
    "Bonecrusher Giant // Stomp",
    "Delver of Secrets // Insectile Aberration",
    "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
    "Fire // Ice",
    "Forest",
    "Hazmat Suit (Used)",
    "Island",
    "Jace, the Mind Sculptor",
    "Lightning Bolt",
    "Lim-Dûl's Vault",
    "Little Girl",
    "Mountain",
    "Mystery Card",
    "Pyroblast",
    "Shock",
    "Sol Ring",
    "Valakut Awakening // Valakut Stoneforge",
];

const RED: [&str; 5] = [
    "Bonecrusher Giant // Stomp",
    "Lightning Bolt",
    "Pyroblast",
    "Shock",
    "Valakut Awakening // Valakut Stoneforge",
];

const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";

/// Both schemas migrated, the fixture imported, and a server over it whose engine runs `caps`.
async fn start(pool: &PgPool, caps: Caps) -> TestServer {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(CARDS.as_bytes()))
        .await
        .unwrap();
    db.importer()
        .import_rulings(rulings(RULINGS.as_bytes()))
        .await
        .unwrap();
    let source = CardSource::new(pool.clone()).with_caps(caps);
    TestServer::start_with_cards(pool.clone(), bot_config(), source).await
}

fn names(page: &Value) -> Vec<String> {
    page["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| card["name"].as_str().unwrap().to_owned())
        .collect()
}

async fn search(server: &TestServer, query: &str) -> Value {
    let reply = get(server.addr, &format!("/api/cards{query}")).await;
    assert_eq!(reply.status, 200, "{query}: {}", reply.text());
    reply.json()
}

/// A query and the cards it must return, in order.
type Case = (&'static str, Vec<&'static str>);

/// Free text.
fn text_cases() -> Vec<Case> {
    vec![
        ("", ALL.to_vec()),
        ("?q=sol%20ring", vec!["Sol Ring"]),
        ("?q=AETHER", vec!["Æther Vial"]),
        (
            "?q=damage",
            vec![
                "Bonecrusher Giant // Stomp",
                "Fire // Ice",
                "Lightning Bolt",
                "Shock",
            ],
        ),
        ("?q=damage&cmc=1", vec!["Lightning Bolt", "Shock"]),
        ("?q=zzzzzz", vec![]),
    ]
}

/// Colors and color identity.
fn color_cases() -> Vec<Case> {
    vec![
        ("?colors=R&colors_match=exactly", RED.to_vec()),
        (
            "?colors=ub",
            vec!["Atraxa, Praetors' Voice", "Lim-Dûl's Vault"],
        ),
        ("?colors=UR&colors_match=exactly", vec!["Fire // Ice"]),
        (
            "?colors=WB&colors_match=overlaps",
            vec![
                "Atraxa, Praetors' Voice",
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Lim-Dûl's Vault",
                "Little Girl",
            ],
        ),
        (
            "?colors=C",
            vec![
                "Æther Vial",
                "Arcane Signet",
                "Forest",
                "Hazmat Suit (Used)",
                "Island",
                "Mountain",
                "Mystery Card",
                "Sol Ring",
            ],
        ),
        (
            "?identity=C",
            vec![
                "Æther Vial",
                "Arcane Signet",
                "Hazmat Suit (Used)",
                "Mystery Card",
                "Sol Ring",
            ],
        ),
        (
            "?identity=u",
            vec![
                "Æther Vial",
                "Arcane Signet",
                "Delver of Secrets // Insectile Aberration",
                "Hazmat Suit (Used)",
                "Island",
                "Jace, the Mind Sculptor",
                "Mystery Card",
                "Sol Ring",
            ],
        ),
    ]
}

/// Types and mana values.
fn value_cases() -> Vec<Case> {
    vec![
        (
            "?type=legendary%20creature",
            vec!["Atraxa, Praetors' Voice"],
        ),
        (
            "?type=PLANESWALKER",
            vec![
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Jace, the Mind Sculptor",
            ],
        ),
        ("?type=basic+land", vec!["Forest", "Island", "Mountain"]),
        (
            "?cmc=0",
            vec!["Forest", "Island", "Mountain", "Mystery Card"],
        ),
        ("?cmc=0.5", vec!["Little Girl"]),
        (
            "?cmc=2-3",
            vec![
                "Arcane Signet",
                "Bonecrusher Giant // Stomp",
                "Hazmat Suit (Used)",
                "Lim-Dûl's Vault",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        (
            "?cmc=4%2B",
            vec![
                "Atraxa, Praetors' Voice",
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Fire // Ice",
                "Jace, the Mind Sculptor",
            ],
        ),
    ]
}

/// Formats, blank values and every filter at once.
fn format_cases() -> Vec<Case> {
    vec![
        ("?format=standard", vec!["Forest", "Island", "Mountain"]),
        ("?format=Standard", vec!["Forest", "Island", "Mountain"]),
        (
            "?format=vintage",
            vec!["Jace, the Mind Sculptor", "Lightning Bolt", "Sol Ring"],
        ),
        (
            "?format=legacy",
            vec![
                "Æther Vial",
                "Fire // Ice",
                "Lightning Bolt",
                "Lim-Dûl's Vault",
            ],
        ),
        ("?format=nonesuch", vec![]),
        ("?q=&colors=&type=&cmc=&format=", ALL.to_vec()),
        (
            "?q=&identity=UR&cmc=1-2&type=instant&format=commander",
            vec![
                "Lightning Bolt",
                "Pyroblast",
                "Shock",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
    ]
}

#[sqlx::test(migrations = false)]
async fn filters_return_exactly_the_cards_they_should(pool: PgPool) {
    let server = start(&pool, Caps::everything()).await;
    let cases = [text_cases(), color_cases(), value_cases(), format_cases()].concat();
    let mut failures = Vec::new();
    for (query, expected) in cases {
        let page = search(&server, query).await;
        let found = names(&page);
        if found != expected || page["total"] != expected.len() {
            failures.push(format!(
                "{query}:\n  expected {expected:?}\n  found    {found:?} (total {})",
                page["total"]
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[sqlx::test(migrations = false)]
async fn free_text_ranks_the_name_before_the_rules_text(pool: PgPool) {
    let server = start(&pool, Caps::everything()).await;
    // "ice" is inside "Fire // Ice" and "Praetors' Voice": both are names, so both match.
    let found = names(&search(&server, "?q=ice").await);
    assert!(found.len() >= 2, "{found:?}");
    assert!(found.contains(&"Fire // Ice".to_owned()), "{found:?}");
    assert!(
        found.contains(&"Atraxa, Praetors' Voice".to_owned()),
        "{found:?}"
    );
    // Without text the order is by name, and an accented letter sorts as its plain one.
    assert_eq!(
        names(&search(&server, "").await)[..2],
        ["Æther Vial", "Arcane Signet"]
    );
}

#[sqlx::test(migrations = false)]
async fn pages_tile_the_result_and_report_the_total(pool: PgPool) {
    let server = start(&pool, Caps::everything()).await;
    let mut seen = Vec::new();
    for page in 1..=3 {
        let body = search(&server, &format!("?per_page=7&page={page}")).await;
        assert_eq!(body["total"], 20);
        assert_eq!(
            (body["page"].as_u64(), body["per_page"].as_u64()),
            (Some(page), Some(7))
        );
        seen.extend(names(&body));
    }
    let last = search(&server, "?per_page=7&page=3").await;
    assert_eq!(names(&last).len(), 6);
    assert_eq!(seen, ALL, "three pages, nothing repeated or skipped");
    let past = search(&server, "?per_page=7&page=4").await;
    assert_eq!((names(&past).len(), past["total"].as_u64()), (0, Some(20)));
}

#[sqlx::test(migrations = false)]
async fn the_page_size_defaults_and_is_capped(pool: PgPool) {
    let server = start(&pool, Caps::everything()).await;
    let page = search(&server, "").await;
    assert_eq!(
        (page["page"].as_u64(), page["per_page"].as_u64()),
        (Some(1), Some(40))
    );
    let capped = search(&server, "?per_page=100000").await;
    assert_eq!(capped["per_page"], 100);
    assert_eq!(names(&capped).len(), 20);
    let one = search(&server, "?per_page=1").await;
    assert_eq!(names(&one), ["Æther Vial"]);
    assert_eq!(one["total"], 20);
}

#[sqlx::test(migrations = false)]
async fn bad_queries_are_400s_that_say_why(pool: PgPool) {
    let server = start(&pool, Caps::everything()).await;
    for (query, fragment) in [
        ("?sort=name", "unknown parameter"),
        ("?colors=WX", "colors is made of"),
        ("?identity=CW", "identity is made of"),
        ("?cmc=lots", "cmc is"),
        ("?page=0", "page is"),
        ("?per_page=0", "per_page is"),
        ("?per_page=many", "per_page is"),
        ("?format=com%20mander", "format is"),
        ("?colors=W&colors_match=some", "colors_match"),
    ] {
        let reply = get(server.addr, &format!("/api/cards{query}")).await;
        assert_eq!(reply.status, 400, "{query}");
        assert_eq!(reply.error_code(), "invalid_request", "{query}");
        let message = reply.json()["message"].as_str().unwrap().to_owned();
        assert!(message.contains(fragment), "{query}: {message}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_hit_carries_what_a_deck_list_needs(pool: PgPool) {
    let server = start(&pool, Caps::none()).await;
    let page = search(&server, "?q=lightning%20bolt").await;
    let hits = page["cards"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    let bolt = &hits[0];
    let keys: BTreeSet<&str> = bolt
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from([
            "color_identity",
            "colors",
            "images",
            "mana_cost",
            "mana_value",
            "name",
            "oracle_id",
            "support",
            "type_line"
        ])
    );
    assert_eq!(bolt["oracle_id"], BOLT);
    assert_eq!(bolt["mana_cost"], "{R}");
    assert_eq!(bolt["mana_value"], 1.0);
    assert_eq!(bolt["type_line"], "Instant");
    assert_eq!(bolt["colors"], json!(["R"]));
    assert_eq!(bolt["color_identity"], json!(["R"]));
    let normal = bolt["images"]["card"]["normal"].as_str().unwrap();
    assert!(
        normal.starts_with("https://cards.scryfall.io/normal/") && normal.contains("e3285e6b"),
        "{normal}"
    );
    assert_eq!(
        bolt["support"], "manual",
        "an engine that runs nothing plays nothing"
    );

    let split = search(&server, "?q=fire%20//%20ice").await;
    let fire = &split["cards"][0];
    assert_eq!(fire["name"], "Fire // Ice");
    assert_eq!(fire["colors"], json!(["U", "R"]));
    assert_eq!(fire["images"]["faces"].as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = false)]
async fn the_support_state_follows_the_rules_and_the_engine(pool: PgPool) {
    for (caps, state) in [(Caps::everything(), "verified"), (Caps::none(), "manual")] {
        let server = start(&pool, caps).await;
        let page = search(&server, "?q=lightning%20bolt").await;
        assert_eq!(page["cards"][0]["support"], state);
        // A card with no rules file is played by hand whatever the engine runs.
        let hazmat = search(&server, "?q=hazmat").await;
        assert_eq!(hazmat["cards"][0]["support"], "manual");
    }
}

#[sqlx::test(migrations = false)]
async fn a_card_page_has_faces_legalities_printings_and_rulings(pool: PgPool) {
    let server = start(&pool, Caps::everything()).await;
    let reply = get(server.addr, &format!("/api/cards/{BOLT}")).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let card = reply.json();
    assert_eq!(card["card"]["name"], "Lightning Bolt");
    assert_eq!(card["card"]["oracle_id"], BOLT);
    assert_eq!(
        card["oracle_text"],
        "Lightning Bolt deals 3 damage to any target."
    );
    assert_eq!(card["faces"].as_array().unwrap().len(), 1);
    assert_eq!(card["faces"][0]["type_line"], "Instant");
    assert_eq!(card["legalities"]["commander"], "legal");
    let printings = card["printings"].as_array().unwrap();
    assert_eq!(printings.len(), 1);
    assert_eq!(
        (
            printings[0]["set"].as_str(),
            printings[0]["collector_number"].as_str()
        ),
        (Some("m10"), Some("146"))
    );
    assert_eq!(card["rulings"].as_array().unwrap().len(), 1);
    assert_eq!(card["reasons"], json!([]));
    assert_eq!(card["card"]["support"], "verified");

    // The id may be spelled in capitals; the page is the same.
    let shouting = get(server.addr, &format!("/api/cards/{}", BOLT.to_uppercase())).await;
    assert_eq!(shouting.status, 200);
    assert_eq!(shouting.json(), card);
}

#[sqlx::test(migrations = false)]
async fn a_card_the_engine_cannot_play_lists_its_clauses_with_spans(pool: PgPool) {
    let server = start(&pool, Caps::none()).await;
    let card = get(server.addr, &format!("/api/cards/{BOLT}")).await.json();
    assert_eq!(card["card"]["support"], "manual");
    let reasons = card["reasons"].as_array().unwrap();
    assert_ne!(reasons.len(), 0);
    assert_eq!(reasons[0]["face"], 0);
    assert_eq!(reasons[0]["span"]["start"], 0);
    assert!(
        reasons[0]["text"]
            .as_str()
            .unwrap()
            .contains("does not run")
    );
}

#[sqlx::test(migrations = false)]
async fn unknown_and_malformed_ids_are_404(pool: PgPool) {
    let server = start(&pool, Caps::everything()).await;
    for path in [
        "/api/cards/not-an-id",
        "/api/cards/%20",
        "/api/cards/00000000-0000-4000-8000-000000000001",
    ] {
        let reply = get(server.addr, path).await;
        assert_eq!(reply.status, 404, "{path}");
        assert_eq!(reply.error_code(), "not_found", "{path}");
    }
}

#[sqlx::test(migrations = false)]
async fn the_card_endpoints_are_read_only_and_public(pool: PgPool) {
    let server = start(&pool, Caps::everything()).await;
    for (method, path) in [
        ("POST", "/api/cards"),
        ("PUT", "/api/cards"),
        ("DELETE", "/api/cards"),
        ("POST", "/api/cards/4457ed35-7c10-48c8-9776-456485fdf070"),
        ("DELETE", "/api/cards/4457ed35-7c10-48c8-9776-456485fdf070"),
    ] {
        let reply = request(server.addr, method, path, &[], b"").await;
        assert_eq!(reply.status, 405, "{method} {path}");
    }
    // No cookie, no account, no origin: a stranger may look.
    assert_eq!(get(server.addr, "/api/cards").await.status, 200);
}

#[tokio::test]
async fn a_server_without_a_database_has_no_cards() {
    let server = TestServer::start(bot_config()).await;
    for path in ["/api/cards", &format!("/api/cards/{BOLT}")] {
        let reply = get(server.addr, path).await;
        assert_eq!(reply.status, 503, "{path}");
        assert_eq!(reply.error_code(), "cards_unavailable");
    }
}
