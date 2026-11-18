//! Goal: endless Delves works end to end on Postgres: a player and a bot ally clear the first floor and the run waits
//! on the cleared floor with what a descent changes; the player's vote to descend (the healthy bot agrees) gives the
//! final boss's loot, and once it is answered floor 2 starts with a new ladder, a heart back and the next fight made,
//! whose intro lists Intensify; a floor vote out of turn is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use net::{Browser, PASSWORD, Reply, TestServer, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn call(
    server: &TestServer,
    method: &str,
    path: &str,
    who: &Browser,
    body: Option<Value>,
) -> Reply {
    let mut headers: Vec<(&str, &str)> = vec![("Cookie", who.cookie.as_str())];
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    request(server.addr, method, path, &headers, body.as_bytes()).await
}

async fn act(server: &TestServer, id: &str, who: &Browser, action: Value) -> Value {
    let path = format!("/api/delves/{id}/act");
    let reply = call(server, "POST", &path, who, Some(action.clone())).await;
    assert_eq!(reply.status, 200, "{action}: {}", reply.text());
    reply.json()
}

async fn read(server: &TestServer, id: &str, who: &Browser) -> Value {
    let reply = call(server, "GET", &format!("/api/delves/{id}"), who, None).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

/// The party wins the fight it is in, as if its game had ended with the heroes on top.
async fn win_fight(pool: &PgPool, id: &str, heroes: usize) {
    let event = json!({"FightEnded": {"won": true, "lives": vec![38; heroes], "spirits": []}});
    sqlx::query(
        "INSERT INTO delve_events (delve_id, seq, event)
         SELECT $1, COALESCE(MAX(seq) + 1, 0), $2 FROM delve_events WHERE delve_id = $1",
    )
    .bind(id.parse::<uuid::Uuid>().unwrap())
    .bind(event)
    .execute(pool)
    .await
    .unwrap();
}

/// The player's answer to their first offer: a rest at a campfire, leaving a shop or the Mirror Pool, else the first
/// choice.
fn answer(offer: &Value) -> Value {
    if offer.get("Campfire").is_some() {
        json!("Rest")
    } else if offer.get("Shop").is_some() || offer == "Duplicate" {
        json!("Skip")
    } else {
        json!({"Take": 0})
    }
}

/// Plays the floor to its end: wins each fight, answers each offer and votes for the first camp.
async fn clear_floor(server: &TestServer, pool: &PgPool, id: &str, who: &Browser) -> Value {
    for _ in 0..80 {
        let view = read(server, id, who).await;
        let run = &view["run"];
        if run["status"] == "FloorCleared" {
            return view;
        }
        if run["status"]["Fight"]["game"].is_string() {
            win_fight(pool, id, 2).await;
        } else if run["status"] == "Room" {
            let offer = run["heroes"][0]["offers"][0].clone();
            if !offer.is_null() {
                act(
                    server,
                    id,
                    who,
                    json!({"kind": "pick", "pick": answer(&offer)}),
                )
                .await;
            }
        } else if run["status"] == "Map" {
            let at = usize::try_from(run["at"].as_u64().unwrap()).unwrap();
            let node = run["map"]["nodes"][at]["next"][0].clone();
            act(server, id, who, json!({"kind": "vote", "node": node})).await;
        } else {
            panic!("unexpected status {}", run["status"]);
        }
    }
    panic!("the floor never ended");
}

#[sqlx::test(migrations = false)]
async fn the_party_clears_a_floor_and_descends(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "diver", PASSWORD).await;
    let body = json!({"heat": 1, "seed": "DEEP"});
    let created = call(&server, "POST", "/api/delves", &host, Some(body)).await;
    assert_eq!(created.status, 201, "{}", created.text());
    let id = created.json()["id"].as_str().unwrap().to_owned();
    act(
        &server,
        &id,
        &host,
        json!({"kind": "add_bot", "level": "Normal"}),
    )
    .await;
    for hero in 0..2 {
        let deck = pg_commander::on(pg_commander::BEAST);
        act(
            &server,
            &id,
            &host,
            json!({"kind": "choose_deck", "hero": hero, "deck": deck}),
        )
        .await;
    }
    let early = call(
        &server,
        "POST",
        &format!("/api/delves/{id}/act"),
        &host,
        Some(json!({"kind": "floor_vote", "descend": true})),
    )
    .await;
    assert_eq!(
        early.status,
        409,
        "no floor vote in the lobby: {}",
        early.text()
    );
    act(&server, &id, &host, json!({"kind": "start"})).await;

    let cleared = clear_floor(&server, &pool, &id, &host).await;
    let run = &cleared["run"];
    assert_eq!(
        (run["floor"].as_u64(), run["floors_cleared"].as_u64()),
        (Some(1), Some(1))
    );
    assert!(
        !cleared["descent"].as_array().unwrap().is_empty(),
        "the choice says what changes"
    );
    let bosses = |run: &Value| -> Vec<Value> {
        run["map"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n.get("boss").cloned())
            .collect()
    };
    let first = bosses(run);

    let looted = act(
        &server,
        &id,
        &host,
        json!({"kind": "floor_vote", "descend": true}),
    )
    .await;
    let run = &looted["run"];
    assert_eq!(run["status"], "Room", "the final boss's loot first");
    assert_eq!(run["floor"], 1);
    assert_eq!(run["descending"], true);
    let offers = &run["heroes"][0]["offers"];
    assert!(offers[0]["Relics"].is_object(), "{offers}");
    let mut after = looted.clone();
    for _ in 0..4 {
        let offer = after["run"]["heroes"][0]["offers"][0].clone();
        if offer.is_null() {
            break;
        }
        let pick = json!({"kind": "pick", "pick": answer(&offer)});
        after = act(&server, &id, &host, pick).await;
    }
    let run = &after["run"];
    assert_eq!(run["floor"], 2, "{}", run["log"]);
    assert!(run["status"]["Fight"].is_object(), "{}", run["status"]);
    assert_eq!(run["floor_votes"], json!({}));
    assert_ne!(bosses(run), first, "a new ladder");
    assert_eq!(after["descent"], json!([]));
    let view = read(&server, &id, &host).await;
    let intensify = view["fight"]["intensify"].as_array().unwrap();
    assert!(!intensify.is_empty(), "{}", view["fight"]);
}
