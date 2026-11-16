//! Goal: on Postgres a new run stores its content generation and each chosen deck's commander facts; a card a hero
//! takes as loot goes to their Satchel, and the next fight's recipe keeps the hero's library as it was with the card in
//! the seat's Satchel, so the game starts with it in the hero's command zone apart from the commanders.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::pg_delve_forfeit::{act, call, fight_run, game_of, read};
use crate::{net, pg_commander};

use mtg_campaign::run::CONTENT;
use net::{Browser, PASSWORD, TestServer, register};
use pg_commander::oracle;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

async fn recipe_of(pool: &PgPool, game: &str) -> Value {
    sqlx::query_scalar("SELECT recipe FROM games WHERE id = $1")
        .bind(Uuid::parse_str(game).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn stored_events(pool: &PgPool, id: &str) -> Vec<Value> {
    sqlx::query_scalar("SELECT event FROM delve_events WHERE delve_id = $1 ORDER BY seq")
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn win_fight(pool: &PgPool, id: &str) {
    let event = json!({"FightEnded": {"won": true, "lives": [30, 30], "spirits": []}});
    sqlx::query(
        "INSERT INTO delve_events (delve_id, seq, event)
         SELECT $1, COALESCE(MAX(seq) + 1, 0), $2 FROM delve_events WHERE delve_id = $1",
    )
    .bind(Uuid::parse_str(id).unwrap())
    .bind(event)
    .execute(pool)
    .await
    .unwrap();
}

/// Answers the host's offers (no relic, the first card, a rest, leaving shops and the pool, the first option) and votes
/// for the first camp until the next fight's game is made; the card taken.
async fn on_to_the_next_fight(server: &TestServer, id: &str, host: &Browser) -> String {
    let mut taken = None;
    for _ in 0..16 {
        let view = read(server, id, host).await;
        if view["run"]["status"]["Fight"]["game"].is_string() {
            return taken.expect("a card was taken");
        }
        let offer = view["run"]["heroes"][0]["offers"].get(0).cloned();
        let action = if let Some(offer) = offer {
            let pick = if offer.get("Relics").is_some()
                || offer.get("Shop").is_some()
                || offer == "Duplicate"
            {
                json!("Skip")
            } else if offer.get("Campfire").is_some() {
                json!("Rest")
            } else {
                if let Some(cards) = offer["Cards"]["cards"].as_array()
                    && taken.is_none()
                {
                    taken = cards.first().and_then(Value::as_str).map(str::to_owned);
                }
                json!({"Take": 0})
            };
            json!({"kind": "pick", "pick": pick})
        } else {
            let at = &view["run"]["at"];
            let node = view["run"]["map"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| &node["id"] == at)
                .map(|node| node["next"][0].clone())
                .unwrap();
            json!({"kind": "vote", "node": node})
        };
        let reply = act(server, id, host, action).await;
        assert_eq!(reply.status, 200, "{}", reply.text());
    }
    panic!("the next fight was never made");
}

#[sqlx::test(migrations = false)]
async fn a_loot_card_waits_in_the_satchel_of_the_next_fight(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "satchel", PASSWORD).await;
    let (id, first) = fight_run(&server, &host, None).await;

    // The run is of this content generation, and each deck brings its commander's facts.
    let stored = stored_events(&pool, &id).await;
    assert!(
        stored.contains(&json!({"Content": {"version": CONTENT}})),
        "{stored:?}"
    );
    let view = read(&server, &id, &host).await;
    let facts = &view["run"]["heroes"][0]["deck"]["commanders"];
    assert_eq!(facts.as_array().unwrap().len(), 1, "{facts}");
    assert!(facts[0]["name"].is_string());

    let before = recipe_of(&pool, &first).await;
    win_fight(&pool, &id).await;
    let card = on_to_the_next_fight(&server, &id, &host).await;
    let view = read(&server, &id, &host).await;
    assert_eq!(view["run"]["heroes"][0]["satchel"], json!([card]));
    let after = recipe_of(&pool, &game_of(&view)).await;
    let (old, new) = (&before["decks"][0], &after["decks"][0]);
    assert_eq!(new["cards"], old["cards"], "the library stays as chosen");
    assert_eq!(old.get("satchel"), None);
    assert_eq!(new["satchel"], json!([oracle(&card)]));
    let refused = call(
        &server,
        "POST",
        &format!("/api/delves/{id}/act"),
        &host,
        Some(json!({"kind": "pick", "pick": {"Duplicate": card}})),
    )
    .await;
    assert_eq!(refused.status, 409, "{}", refused.text());
}
