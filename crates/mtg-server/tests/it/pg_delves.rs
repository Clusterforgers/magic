//! Goal: Delves work end to end on Postgres: a player creates a run and hosts its lobby, a friend joins by its code,
//! the host adds a bot ally and chooses its deck while each player chooses their own, only the host starts, and the
//! start leads the party to the first boss, whose fight is a game made at once, and made again at no cost when an admin
//! ends it unplayed; the content the web draws lists the bosses and custom cards; a party of four meets the boss with a
//! minion seat; strangers and unknown runs are refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use net::{Browser, PASSWORD, Reply, TestServer, register, request};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn call(
    server: &TestServer,
    method: &str,
    path: &str,
    who: Option<&Browser>,
    body: Option<Value>,
) -> Reply {
    let mut headers: Vec<(&str, &str)> = Vec::new();
    if let Some(browser) = who {
        headers.push(("Cookie", browser.cookie.as_str()));
    }
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    request(server.addr, method, path, &headers, body.as_bytes()).await
}

async fn act(server: &TestServer, id: &str, who: &Browser, action: Value) -> Reply {
    call(
        server,
        "POST",
        &format!("/api/delves/{id}/act"),
        Some(who),
        Some(action),
    )
    .await
}

#[sqlx::test(migrations = false)]
async fn a_party_gathers_and_the_start_makes_the_first_boss_fight(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let guest = register(server.addr, "guest", PASSWORD).await;
    let stranger = register(server.addr, "stranger", PASSWORD).await;

    let reply = call(
        &server,
        "POST",
        "/api/delves",
        Some(&host),
        Some(json!({"heat": 0, "seed": "DELVE"})),
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let created = reply.json();
    let id = created["id"].as_str().unwrap().to_owned();
    let code = created["code"].as_str().unwrap().to_owned();
    assert_eq!(created["seed"], "DELVE");
    assert_eq!(created["me"], 0);
    assert_eq!(created["host"], true);
    assert_eq!(created["run"]["status"], "Lobby");

    let joined = call(
        &server,
        "POST",
        &format!("/api/delves/join/{code}"),
        Some(&guest),
        None,
    )
    .await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    assert_eq!(joined.json()["me"], 1);

    let bot = act(
        &server,
        &id,
        &guest,
        json!({"kind": "add_bot", "level": "Normal"}),
    )
    .await;
    assert_eq!(bot.status, 403, "only the host adds allies");
    let bot = act(
        &server,
        &id,
        &host,
        json!({"kind": "add_bot", "level": "Normal"}),
    )
    .await;
    assert_eq!(bot.status, 200, "{}", bot.text());
    assert_eq!(bot.json()["run"]["heroes"].as_array().unwrap().len(), 3);

    let other = act(
        &server,
        &id,
        &guest,
        json!({"kind": "choose_deck", "hero": 0, "deck": pg_commander::on(pg_commander::RED)}),
    )
    .await;
    assert_eq!(other.status, 403, "a player chooses only their own deck");
    for (who, hero, code) in [
        (&host, 0, pg_commander::BEAST),
        (&guest, 1, pg_commander::RED),
        (&host, 2, pg_commander::BEAR),
    ] {
        let chosen = act(
            &server,
            &id,
            who,
            json!({"kind": "choose_deck", "hero": hero, "deck": pg_commander::on(code)}),
        )
        .await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    let start = act(&server, &id, &guest, json!({"kind": "start"})).await;
    assert_eq!(start.status, 403);
    let start = act(&server, &id, &stranger, json!({"kind": "start"})).await;
    assert_eq!(start.status, 403, "a stranger is not in the party");

    let start = act(&server, &id, &host, json!({"kind": "start"})).await;
    assert_eq!(start.status, 200, "{}", start.text());
    let view = start.json();
    let game = view["run"]["status"]["Fight"]["game"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        !game.is_empty(),
        "the first boss fight is made: {}",
        view["run"]["status"]
    );
    assert_eq!(view["run"]["at"], 0);
    assert!(view["boss"].is_string());

    // The guest sees the run in their list and the same fight on reading it.
    let list = call(&server, "GET", "/api/delves", Some(&guest), None)
        .await
        .json();
    assert_eq!(list["delves"][0]["id"], id.as_str());
    assert_eq!(list["delves"][0]["status"], "fight");
    let again = call(
        &server,
        "GET",
        &format!("/api/delves/{id}"),
        Some(&guest),
        None,
    )
    .await
    .json();
    assert_eq!(again["run"]["status"]["Fight"]["game"], game.as_str());
    assert_eq!(again["me"], 1);

    // An admin ends the fight: it was not played, so the next read makes a new one and the run loses nothing.
    let ended = server
        .registry
        .end_by_admin(game.parse().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(ended, mtg_server::registry::AdminEnd::Ended);
    let retried = call(
        &server,
        "GET",
        &format!("/api/delves/{id}"),
        Some(&host),
        None,
    )
    .await
    .json();
    let fresh = retried["run"]["status"]["Fight"]["game"].as_str().unwrap();
    assert_ne!(fresh, game);
    assert!(server.registry.handle(fresh.parse().unwrap()).is_some());
    assert_eq!(retried["run"]["hearts"], again["run"]["hearts"]);
    assert_eq!(retried["run"]["heroes"], again["run"]["heroes"]);

    let missing = call(
        &server,
        "GET",
        "/api/delves/00000000-0000-0000-0000-000000000000",
        Some(&guest),
        None,
    )
    .await;
    assert_eq!(missing.status, 404);
}

#[sqlx::test(migrations = false)]
async fn a_party_of_four_meets_the_boss_and_a_minion(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let created = call(
        &server,
        "POST",
        "/api/delves",
        Some(&host),
        Some(json!({"heat": 0, "seed": "HORDE"})),
    )
    .await
    .json();
    let id = created["id"].as_str().unwrap().to_owned();
    for _ in 0..3 {
        let bot = act(
            &server,
            &id,
            &host,
            json!({"kind": "add_bot", "level": "Normal"}),
        )
        .await;
        assert_eq!(bot.status, 200, "{}", bot.text());
    }
    for hero in 0..4 {
        let chosen = act(
            &server,
            &id,
            &host,
            json!({"kind": "choose_deck", "hero": hero, "deck": pg_commander::on(pg_commander::BEAST)}),
        )
        .await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    let start = act(&server, &id, &host, json!({"kind": "start"})).await;
    assert_eq!(start.status, 200, "{}", start.text());
    let view = start.json();
    let game = view["run"]["status"]["Fight"]["game"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        !game.is_empty(),
        "the fight is made: {}",
        view["run"]["status"]
    );
    let table = call(
        &server,
        "GET",
        &format!("/api/games/{game}"),
        Some(&host),
        None,
    )
    .await
    .json();
    let names: Vec<&str> = table["seats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|seat| seat["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names.len(),
        6,
        "four heroes, the boss and one minion: {names:?}"
    );
    let content = mtg_campaign::content().unwrap();
    let theme = &content.themes[0];
    assert!(theme.bosses.iter().any(|b| b.name == names[4]), "{names:?}");
    assert!(
        theme.minions.iter().any(|m| m.name == names[5]),
        "{names:?}"
    );
}

#[sqlx::test(migrations = false)]
async fn the_content_lists_the_bosses_and_custom_cards(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let content = call(&server, "GET", "/api/delves/content", None, None).await;
    assert_eq!(content.status, 200);
    let content = content.json();
    assert_eq!(content["bosses"].as_array().unwrap().len(), 4);
    let cards = content["cards"].as_array().unwrap();
    assert!(cards.iter().any(|c| c["kind"] == "Haunt"));
    assert!(cards.iter().any(|c| c["role"] == "Risky"));
    let bad = call(
        &server,
        "POST",
        "/api/delves",
        Some(&register(server.addr, "heater", PASSWORD).await),
        Some(json!({"heat": 7})),
    )
    .await;
    assert_eq!(bad.status, 400);
}
