//! Goal: the camp vote works end to end on Postgres for a party of two or three players with a bot ally: after the
//! first boss falls and the loot is taken, the run waits on the map; each vote is accepted and seen by the others on
//! their next read; the run waits for every player (bots follow the majority, a tie goes to the torch bearer) and then
//! enters the chosen camp; after the camp the party walks into the next boss with no second vote; a vote out of turn
//! is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use net::{Browser, PASSWORD, Reply, TestServer, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Each case: a name, the players, each player's vote (0 or 1, the first or second camp) and the camp that wins.
/// The torch bearer after the first boss is the second hero.
const CASES: [(&str, usize, &[usize], usize); 4] = [
    ("split", 2, &[0, 1], 1),
    ("agree", 2, &[1, 1], 1),
    ("majority", 3, &[0, 0, 1], 0),
    ("against", 3, &[0, 1, 0], 0),
];

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

async fn act(server: &TestServer, id: &str, who: &Browser, action: Value) -> Reply {
    let path = format!("/api/delves/{id}/act");
    call(server, "POST", &path, who, Some(action)).await
}

async fn read(server: &TestServer, id: &str, who: &Browser) -> Value {
    let reply = call(server, "GET", &format!("/api/delves/{id}"), who, None).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

/// The answer a player gives their first offer: a rest at a campfire, leaving a shop, else the first choice.
fn answer(offer: &Value) -> Value {
    if offer == "Campfire" {
        json!("Rest")
    } else if offer.get("Shop").is_some() {
        json!("Skip")
    } else {
        json!({"Take": 0})
    }
}

/// Answers every offer of `who` until none is left.
async fn take_all(server: &TestServer, id: &str, who: &Browser) {
    let mut view = read(server, id, who).await;
    for _ in 0..8 {
        let me = usize::try_from(view["me"].as_u64().unwrap()).unwrap();
        let Some(offer) = view["run"]["heroes"][me]["offers"].get(0).cloned() else {
            return;
        };
        let pick = json!({"kind": "pick", "pick": answer(&offer)});
        let reply = act(server, id, who, pick).await;
        assert_eq!(reply.status, 200, "{offer}: {}", reply.text());
        view = reply.json();
    }
    panic!("offers never ran out: {}", view["run"]["heroes"]);
}

/// The party wins the fight it is in, as if its game had ended with the heroes on top.
async fn win_fight(pool: &PgPool, id: &str, heroes: usize) {
    let event = json!({"FightEnded": {"won": true, "lives": vec![30; heroes], "spirits": []}});
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

fn index(value: &Value) -> usize {
    usize::try_from(value.as_u64().unwrap()).unwrap()
}

#[sqlx::test(migrations = false)]
async fn the_party_votes_for_a_camp_and_moves_on(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    for (name, players, votes, winner) in CASES {
        vote_through(&server, &pool, name, players, votes, winner).await;
    }
}

/// Two or three players and a bot ally clear the first boss, vote for a camp and walk on to the next boss.
async fn vote_through(
    server: &TestServer,
    pool: &PgPool,
    name: &str,
    players: usize,
    votes: &[usize],
    winner: usize,
) {
    let mut party = Vec::new();
    for at in 0..players {
        party.push(register(server.addr, &format!("{name}{at}"), PASSWORD).await);
    }
    let host = &party[0];
    let body = json!({"heat": 0, "seed": "VOTES"});
    let created = call(server, "POST", "/api/delves", host, Some(body))
        .await
        .json();
    let id = created["id"].as_str().unwrap().to_owned();
    let code = created["code"].as_str().unwrap().to_owned();
    for guest in &party[1..] {
        let path = format!("/api/delves/join/{code}");
        let joined = call(server, "POST", &path, guest, None).await;
        assert_eq!(joined.status, 200, "{}", joined.text());
    }
    let bot = act(
        server,
        &id,
        host,
        json!({"kind": "add_bot", "level": "Normal"}),
    )
    .await;
    assert_eq!(bot.status, 200, "{}", bot.text());
    let heroes = players + 1;
    for hero in 0..heroes {
        let who = party.get(hero).unwrap_or(host);
        let deck = pg_commander::on(pg_commander::BEAST);
        let choose = json!({"kind": "choose_deck", "hero": hero, "deck": deck});
        let chosen = act(server, &id, who, choose).await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    let start = act(server, &id, host, json!({"kind": "start"})).await;
    assert_eq!(start.status, 200, "{}", start.text());
    let early = act(server, &id, host, json!({"kind": "vote", "node": 1})).await;
    assert_eq!(
        early.status,
        409,
        "no vote during a fight: {}",
        early.text()
    );

    win_fight(pool, &id, heroes).await;
    for who in &party {
        take_all(server, &id, who).await;
    }
    let view = read(server, &id, host).await;
    assert_eq!(view["run"]["status"], "Map", "{}", view["run"]["log"]);
    let at = index(&view["run"]["at"]);
    let choices: Vec<u64> = view["run"]["map"]["nodes"][at]["next"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_u64().unwrap())
        .collect();
    assert_eq!(choices.len(), 2, "two camps after a boss");

    for (hero, vote) in votes.iter().enumerate() {
        let node = choices[*vote];
        let ballot = json!({"kind": "vote", "node": node});
        let reply = act(server, &id, &party[hero], ballot).await;
        assert_eq!(reply.status, 200, "{}", reply.text());
        if hero + 1 < players {
            assert_eq!(
                reply.json()["run"]["status"],
                "Map",
                "the run waits for every player"
            );
            // The next player sees the vote on their next read; the bot has not voted yet.
            let other = read(server, &id, &party[hero + 1]).await;
            assert_eq!(other["run"]["votes"][hero.to_string()], node);
            assert!(other["run"]["votes"].get(players.to_string()).is_none());
        }
    }
    let after = read(server, &id, &party[players - 1]).await;
    assert_eq!(
        after["run"]["status"], "Room",
        "{name}: {}",
        after["run"]["log"]
    );
    assert_eq!(after["run"]["at"], choices[winner], "{name}");
    assert_eq!(after["run"]["votes"], json!({}));

    // After the camp the only way on is the next boss: the party walks in and its fight is made, with no vote.
    for who in &party {
        take_all(server, &id, who).await;
    }
    let fight = read(server, &id, host).await;
    let status = &fight["run"]["status"];
    assert!(
        status["Fight"]["game"].is_string(),
        "{name}: the next boss fight: {status}"
    );
    let room = index(&fight["run"]["at"]);
    assert_eq!(fight["run"]["map"]["nodes"][room]["kind"], "boss");
}
