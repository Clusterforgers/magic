//! Goal: endless Delves works end to end on Postgres for a lone player, a pair and a party of four: the player and
//! bot allies clear the first floor (an event's card cost answered as the web answers it) and the run waits on the
//! cleared floor with what a descent changes; the player's vote to descend (healthy bots agree) gives the final boss's
//! loot, and once it is answered floor 2 starts with a new ladder, a heart back and the next fight made,
//! whose intro shows the innate at the deeper level, in the other theme whose boss the fight is; a floor vote out of
//! turn is refused.
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
    let body = body.map_or_default(|b| b.to_string());
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

/// The player's answer to their first offer: a rest at a campfire, leaving a shop or the Mirror Pool, giving up the
/// first card the view lists when an event costs one, else the first choice.
fn answer(offer: &Value, view: &Value) -> Value {
    if offer.get("Campfire").is_some() {
        json!("Rest")
    } else if offer.get("Shop").is_some() || offer == "Duplicate" {
        json!("Skip")
    } else if offer == "Discard" {
        let card = view["mirror"][0].clone();
        assert!(card.is_string(), "a card to give up: {view}");
        json!({ "Discard": card })
    } else {
        json!({"Take": 0})
    }
}

/// Plays the floor to its end: wins each fight, answers each offer and votes for the first camp.
async fn clear_floor(
    server: &TestServer,
    pool: &PgPool,
    id: &str,
    who: &Browser,
    heroes: usize,
) -> Value {
    for _ in 0..80 {
        let view = read(server, id, who).await;
        let run = &view["run"];
        if run["status"] == "FloorCleared" {
            return view;
        }
        if run["status"]["Fight"]["game"].is_string() {
            win_fight(pool, id, heroes).await;
        } else if run["status"] == "Room" {
            let offer = run["heroes"][0]["offers"][0].clone();
            if !offer.is_null() {
                act(
                    server,
                    id,
                    who,
                    json!({"kind": "pick", "pick": answer(&offer, &view)}),
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
async fn a_lone_player_clears_a_floor_and_descends(pool: PgPool) {
    descend(pool, 0).await;
}

#[sqlx::test(migrations = false)]
async fn the_party_clears_a_floor_and_descends(pool: PgPool) {
    descend(pool, 1).await;
}

#[sqlx::test(migrations = false)]
async fn a_party_of_four_clears_a_floor_and_descends(pool: PgPool) {
    descend(pool, 3).await;
}

/// Opens a run on the fixed seed with `allies` bots, every hero on the same deck, refuses a floor vote in the lobby
/// and starts it; gives the run's id.
async fn open(server: &TestServer, host: &Browser, allies: usize) -> String {
    let body = json!({"heat": 1, "seed": "DEEP"});
    let created = call(server, "POST", "/api/delves", host, Some(body)).await;
    assert_eq!(created.status, 201, "{}", created.text());
    let id = created.json()["id"].as_str().unwrap().to_owned();
    for _ in 0..allies {
        let bot = json!({"kind": "add_bot", "level": "Normal"});
        act(server, &id, host, bot).await;
    }
    for hero in 0..=allies {
        let deck = pg_commander::on(pg_commander::BEAST);
        let choose = json!({"kind": "choose_deck", "hero": hero, "deck": deck});
        act(server, &id, host, choose).await;
    }
    let early = call(
        server,
        "POST",
        &format!("/api/delves/{id}/act"),
        host,
        Some(json!({"kind": "floor_vote", "descend": true})),
    )
    .await;
    assert_eq!(
        early.status,
        409,
        "no floor vote in the lobby: {}",
        early.text()
    );
    act(server, &id, host, json!({"kind": "start"})).await;
    id
}

/// A player and `allies` bots clear floor 1 on the same seed and descend into floor 2.
async fn descend(pool: PgPool, allies: usize) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "diver", PASSWORD).await;
    let id = open(&server, &host, allies).await;

    let cleared = clear_floor(&server, &pool, &id, &host, allies + 1).await;
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
        let pick = json!({"kind": "pick", "pick": answer(&offer, &after)});
        after = act(&server, &id, &host, pick).await;
    }
    let run = &after["run"];
    assert_eq!(run["floor"], 2, "{}", run["log"]);
    assert!(run["status"]["Fight"].is_object(), "{}", run["status"]);
    assert_eq!(run["floor_votes"], json!({}));
    assert_ne!(bosses(run), first, "a new ladder");
    assert_eq!(after["descent"], json!([]));
    let view = read(&server, &id, &host).await;
    // Four bosses fell, so the innate stands at level III or higher.
    let level = view["fight"]["innate"]["level"].as_u64().unwrap();
    assert!(level >= 3, "{}", view["fight"]);

    // Floor 2 lies in the other theme: its name heads the view and its boss's game is made.
    let content = mtg_campaign::content().unwrap();
    let run = &view["run"];
    let first = run["theme"].as_str().unwrap();
    let floor = run["floor_theme"].as_str().unwrap();
    assert_ne!(floor, first, "a descent leads into another theme");
    assert_eq!(run["theme_from"], first);
    let theme = content.theme(floor).unwrap();
    assert_eq!(view["theme"], theme.name.as_str());
    assert!(
        run["status"]["Fight"]["game"].is_string(),
        "the fight is made"
    );
    let boss = theme
        .boss(view["boss"].as_str().unwrap())
        .expect("a boss of the floor's theme");
    assert_eq!(view["fight"]["boss"], boss.name.as_str());
    assert!(
        run["log"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line.as_str().unwrap().contains("descends into")),
        "{}",
        run["log"]
    );
}
