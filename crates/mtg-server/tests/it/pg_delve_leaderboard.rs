//! Goal: on Postgres the Delves leaderboard lists one board per Heat from Heroic down, each with every player's deepest
//! run once, deeper first, with the player's commanders, party size and day; a run with no boss beaten and a bot ally
//! never stand on it, a run that moves on moves its entry, and a caller who is not signed in is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::pg_delve_forfeit::{act, call, read};
use crate::{net, pg_commander};

use net::{Browser, PASSWORD, TestServer, register};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

/// A started run at `heat`: the host with a bot ally, both on the bear deck.
async fn started(server: &TestServer, host: &Browser, heat: u8) -> String {
    let body = json!({"heat": heat});
    let created = call(server, "POST", "/api/delves", host, Some(body)).await;
    assert_eq!(created.status, 201, "{}", created.text());
    let id = created.json()["id"].as_str().unwrap().to_owned();
    let bot = act(
        server,
        &id,
        host,
        json!({"kind": "add_bot", "level": "Normal"}),
    )
    .await;
    assert_eq!(bot.status, 200, "{}", bot.text());
    for hero in 0..2 {
        let deck = pg_commander::on(pg_commander::BEAR);
        let chosen = act(
            server,
            &id,
            host,
            json!({"kind": "choose_deck", "hero": hero, "deck": deck}),
        )
        .await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    let start = act(server, &id, host, json!({"kind": "start"})).await;
    assert_eq!(start.status, 200, "{}", start.text());
    id
}

/// Wins the fight the run is in, then answers offers and votes until the next fight's game is made; an event's card
/// cost gives up the first Satchel card.
async fn win(server: &TestServer, pool: &PgPool, id: &str, host: &Browser) {
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
    for _ in 0..16 {
        let view = read(server, id, host).await;
        let run = &view["run"];
        if run["status"]["Fight"]["game"].is_string() {
            return;
        }
        let action = if run["status"] == "Map" {
            let at = usize::try_from(run["at"].as_u64().unwrap()).unwrap();
            json!({"kind": "vote", "node": run["map"]["nodes"][at]["next"][0]})
        } else {
            let offer = &run["heroes"][0]["offers"][0];
            let pick = if offer.get("Campfire").is_some() {
                json!("Rest")
            } else if offer.get("Shop").is_some() || offer == "Duplicate" {
                json!("Skip")
            } else if offer == "Discard" {
                json!({"Discard": run["heroes"][0]["satchel"][0]})
            } else {
                json!({"Take": 0})
            };
            json!({"kind": "pick", "pick": pick})
        };
        let reply = act(server, id, host, action).await;
        assert_eq!(reply.status, 200, "{}", reply.text());
    }
    panic!("the next fight never came");
}

async fn board(server: &TestServer, who: &Browser) -> Value {
    let reply = call(server, "GET", "/api/delves/leaderboard", who, None).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

/// Each entry of the board at `heat` as (player, floors, bosses).
fn rows(board: &Value, heat: u64) -> Vec<(String, u64, u64)> {
    let board = board["boards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["heat"] == heat)
        .unwrap();
    board["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["player"].as_str().unwrap().to_owned(),
                e["floors"].as_u64().unwrap(),
                e["bosses"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[sqlx::test(migrations = false)]
async fn the_leaderboard_ranks_each_players_deepest_run_per_heat(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let ana = register(server.addr, "ana", PASSWORD).await;
    let bo = register(server.addr, "bodil", PASSWORD).await;
    let empty = board(&server, &ana).await;
    let heats: Vec<u64> = empty["boards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["heat"].as_u64().unwrap())
        .collect();
    assert_eq!(heats, vec![3, 2, 1, 0], "the greatest Heat first");

    let shallow = started(&server, &ana, 2).await;
    win(&server, &pool, &shallow, &ana).await;
    let deep = started(&server, &ana, 2).await;
    win(&server, &pool, &deep, &ana).await;
    let casual = started(&server, &ana, 0).await;
    win(&server, &pool, &casual, &ana).await;
    started(&server, &ana, 1).await;
    let other = started(&server, &bo, 2).await;
    win(&server, &pool, &other, &bo).await;

    let got = board(&server, &bo).await;
    assert_eq!(
        rows(&got, 2),
        vec![("ana".to_owned(), 0, 1), ("bodil".to_owned(), 0, 1)],
        "one entry per player"
    );
    assert_eq!(rows(&got, 1), vec![], "no boss beaten, no entry");
    assert_eq!(rows(&got, 0), vec![("ana".to_owned(), 0, 1)]);
    let entry = &got["boards"][0]["entries"][0];
    assert_eq!(entry["party"], 2);
    assert!(
        !entry["commanders"].as_array().unwrap().is_empty(),
        "{entry}"
    );
    assert_eq!(entry["date"].as_str().unwrap().len(), 10, "{entry}");

    // The deeper run moves on, and its entry with it.
    win(&server, &pool, &deep, &ana).await;
    let later = board(&server, &ana).await;
    assert_eq!(
        rows(&later, 2),
        vec![("ana".to_owned(), 0, 2), ("bodil".to_owned(), 0, 1)]
    );

    let stranger = Browser {
        cookie: String::new(),
        ..ana.clone()
    };
    let refused = call(&server, "GET", "/api/delves/leaderboard", &stranger, None).await;
    assert_eq!(refused.status, 401, "{}", refused.text());
}
