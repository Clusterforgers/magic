//! Goal: a Delves fight is made from the decks the party chose in the run's lobby: its recipe plays each hero's deck
//! and names the run, its lookup names the run for the table's way back, and a seat cannot swap its deck in the game's
//! lobby; a hero whose deck is deleted holds the fight with the reason on the run page, may then (and only then)
//! choose another deck, and the fight is made with it; a hero whose deck still plays cannot change it mid-run.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use net::{Browser, PASSWORD, Reply, TestServer, register, request};
use pg_commander::oracle;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

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
    call(
        server,
        "POST",
        &format!("/api/delves/{id}/act"),
        who,
        Some(action),
    )
    .await
}

async fn read(server: &TestServer, id: &str, who: &Browser) -> Value {
    call(server, "GET", &format!("/api/delves/{id}"), who, None)
        .await
        .json()
}

fn game_of(view: &Value) -> Option<String> {
    view["run"]["status"]["Fight"]["game"]
        .as_str()
        .map(str::to_owned)
}

/// A Commander deck of the guest's own: Questing Beast and 99 Forests; its id.
async fn own_deck(server: &TestServer, who: &Browser) -> String {
    let body = json!({"name": "Own Beasts", "format": "commander", "notes": "", "cards": [
        {"zone": "commander", "oracle_id": oracle("Questing Beast"), "printing_id": null, "count": 1},
        {"zone": "main", "oracle_id": oracle("Forest"), "printing_id": null, "count": 99},
    ]});
    let reply = who.post(server.addr, "/api/decks", &body.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

async fn recipe_of(pool: &PgPool, game: &str) -> Value {
    sqlx::query_scalar("SELECT recipe FROM games WHERE id = $1")
        .bind(Uuid::parse_str(game).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn the_fight_plays_the_decks_the_party_chose_and_waits_for_a_lost_one(pool: PgPool) {
    let server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let guest = register(server.addr, "guest", PASSWORD).await;
    let created = call(
        &server,
        "POST",
        "/api/delves",
        &host,
        Some(json!({"heat": 0, "seed": "DECKS"})),
    )
    .await;
    assert_eq!(created.status, 201, "{}", created.text());
    let created = created.json();
    let id = created["id"].as_str().unwrap().to_owned();
    let code = created["code"].as_str().unwrap().to_owned();
    let joined = call(
        &server,
        "POST",
        &format!("/api/delves/join/{code}"),
        &guest,
        None,
    )
    .await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    let mine = own_deck(&server, &guest).await;
    for (who, hero, deck) in [
        (&host, 0, pg_commander::on(pg_commander::BEAR)),
        (&guest, 1, mine.clone()),
    ] {
        let chosen = act(
            &server,
            &id,
            who,
            json!({"kind": "choose_deck", "hero": hero, "deck": deck}),
        )
        .await;
        assert_eq!(chosen.status, 200, "{}", chosen.text());
    }
    let start = act(&server, &id, &host, json!({"kind": "start"})).await;
    assert_eq!(start.status, 200, "{}", start.text());
    let view = start.json();
    assert_eq!(view["deck_problems"], json!([]));
    let game = game_of(&view).expect("the first fight is made at the start");

    // The recipe plays each hero's own deck and names the run.
    let recipe = recipe_of(&pool, &game).await;
    assert_eq!(recipe["campaign"]["run"], id.as_str());
    let decks = recipe["decks"].as_array().unwrap();
    assert!(
        decks[0].to_string().contains(&oracle("Grizzly Bears")),
        "{}",
        decks[0]
    );
    assert!(
        decks[1].to_string().contains(&oracle("Questing Beast")),
        "{}",
        decks[1]
    );

    // The lookup names the run, and the game's lobby keeps the run's decks.
    let info = call(&server, "GET", &format!("/api/games/{game}"), &guest, None)
        .await
        .json();
    assert_eq!(info["delve"], id.as_str());
    let swap = pg_commander::choose(
        &server,
        &guest,
        &game,
        1,
        &pg_commander::on(pg_commander::BEAST),
    )
    .await;
    assert_eq!(swap.status, 409, "{}", swap.text());
    assert_eq!(swap.json()["error"], "deck_fixed");

    // A deck that still plays does not change mid-run.
    let early = act(
        &server,
        &id,
        &guest,
        json!({"kind": "choose_deck", "hero": 1, "deck": pg_commander::on(pg_commander::BEAST)}),
    )
    .await;
    assert_eq!(early.status, 409, "{}", early.text());

    // The fight ends unplayed and the guest deletes their deck: the next fight waits and says why.
    server
        .registry
        .end_by_admin(game.parse().unwrap(), None)
        .await
        .unwrap();
    let deleted = guest
        .delete(server.addr, &format!("/api/decks/{mine}"))
        .await;
    assert!(deleted.status < 300, "{}", deleted.text());
    let waiting = read(&server, &id, &host).await;
    assert_eq!(game_of(&waiting), None, "{}", waiting["run"]["status"]);
    assert_eq!(waiting["deck_problems"][0]["hero"], 1);
    assert!(
        waiting["deck_problems"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("gone")
    );

    // The host's deck still plays, so only the guest may choose again; then the fight is made.
    let host_swap = act(
        &server,
        &id,
        &host,
        json!({"kind": "choose_deck", "hero": 0, "deck": pg_commander::on(pg_commander::BEAST)}),
    )
    .await;
    assert_eq!(host_swap.status, 409, "{}", host_swap.text());
    let replaced = act(
        &server,
        &id,
        &guest,
        json!({"kind": "choose_deck", "hero": 1, "deck": pg_commander::on(pg_commander::BEAST)}),
    )
    .await;
    assert_eq!(replaced.status, 200, "{}", replaced.text());
    let view = replaced.json();
    assert_eq!(view["deck_problems"], json!([]));
    let fresh = game_of(&view).expect("the fight is made with the new deck");
    assert_ne!(fresh, game);
    assert_eq!(
        view["run"]["heroes"][1]["deck"]["id"],
        pg_commander::on(pg_commander::BEAST)
    );
}
