//! Goal: teams are chosen on the create call and set again by the host in the lobby with `PUT /api/games/{id}/teams`: the game's info shows them, another seat may not set them, a player puts their own seat on a team with `PUT /api/games/{id}/seats/{n}/team` and the host any seat, the lobby keeps any grouping while players choose, an assignment that names a seat or team the table lacks is a 400 naming `teams`, and a started game keeps the ones it began with.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use net::lobby::{bearer, body, call, make, start};
use net::{TestServer, bot_config};
use rstest::rstest;
use serde_json::{Value, json};

async fn server() -> TestServer {
    let mut config = bot_config();
    config.auth.limits.lobby_requests_per_ip = 100_000;
    TestServer::start(config).await
}

async fn info(server: &TestServer, game: &str) -> Value {
    let reply = call(server.addr, "GET", &format!("/api/games/{game}"), None, &[]).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

async fn put_teams(server: &TestServer, game: &str, token: &str, teams: &Value) -> net::Reply {
    call(
        server.addr,
        "PUT",
        &format!("/api/games/{game}/teams"),
        Some(&json!({ "teams": teams }).to_string()),
        &[("Authorization", &bearer(token))],
    )
    .await
}

#[rstest]
#[case::giant(4, json!({"mode": "two_headed_giant", "seats": [0, 0, 1, 1]}), 201)]
#[case::three_v_three(6, json!({"mode": "teams", "seats": [0, 1, 0, 1, 0, 1]}), 201)]
#[case::giant_of_six(6, json!({"mode": "two_headed_giant", "seats": [0, 0, 1, 1, 2, 2]}), 400)]
#[case::seats_missing(4, json!({"mode": "teams", "seats": [0, 1]}), 400)]
#[tokio::test]
async fn the_create_call_takes_teams_that_fit_the_table(
    #[case] seats: usize,
    #[case] teams: Value,
    #[case] status: u16,
) {
    let server = server().await;
    let mut request = body("Ann", seats, seats, "invite");
    request["house_rules"] = json!({ "teams": teams });
    let reply = call(
        server.addr,
        "POST",
        "/api/games",
        Some(&request.to_string()),
        &[],
    )
    .await;
    assert_eq!(reply.status, status, "{}", reply.text());
    if status == 400 {
        assert_eq!(reply.json()["field"], "teams");
    } else {
        let game = reply.json()["game"].as_str().unwrap().to_owned();
        assert_eq!(info(&server, &game).await["house_rules"]["teams"], teams);
    }
}

#[tokio::test]
async fn the_host_sets_teams_in_the_lobby_until_the_game_starts() {
    let server = server().await;
    let game = make(server.addr, "Ann", 4, 4, "invite").await;
    let giant = json!({"mode": "two_headed_giant", "seats": [0, 1, 0, 1]});

    let other = put_teams(&server, &game.id, &game.tokens[1], &giant).await;
    assert_eq!(other.status, 403, "{}", other.text());
    let nowhere = json!({"mode": "teams", "seats": [0, 1, 0, 4]});
    let refused = put_teams(&server, &game.id, game.host(), &nowhere).await;
    assert_eq!(refused.status, 400, "{}", refused.text());
    assert_eq!(refused.json()["field"], "teams");
    assert!(info(&server, &game.id).await["house_rules"].is_null());
    // While players choose, any grouping is kept as it is; sizes are checked at the start.
    let lonely = json!({"mode": "teams", "seats": [0, 0, 0, 0]});
    let kept = put_teams(&server, &game.id, game.host(), &lonely).await;
    assert_eq!(kept.status, 200, "{}", kept.text());
    assert_eq!(
        info(&server, &game.id).await["house_rules"]["teams"],
        lonely
    );

    let set = put_teams(&server, &game.id, game.host(), &giant).await;
    assert_eq!(set.status, 200, "{}", set.text());
    assert_eq!(set.json()["teams"], giant);
    assert_eq!(info(&server, &game.id).await["house_rules"]["teams"], giant);

    // A player moves their own seat, the host any seat, and nobody another player's seat.
    let seat_team = |seat: usize| format!("/api/games/{}/seats/{seat}/team", game.id);
    let team = |n: u32| Some(json!({ "team": n }).to_string());
    let own = call(
        server.addr,
        "PUT",
        &seat_team(2),
        team(1).as_deref(),
        &[("Authorization", &bearer(&game.tokens[2]))],
    )
    .await;
    assert_eq!(own.status, 200, "{}", own.text());
    assert_eq!(own.json()["teams"]["seats"], json!([0, 1, 1, 1]));
    let others = call(
        server.addr,
        "PUT",
        &seat_team(3),
        team(0).as_deref(),
        &[("Authorization", &bearer(&game.tokens[2]))],
    )
    .await;
    assert_eq!(others.status, 403, "{}", others.text());
    let host = call(
        server.addr,
        "PUT",
        &seat_team(3),
        team(0).as_deref(),
        &[("Authorization", &bearer(game.host()))],
    )
    .await;
    assert_eq!(host.status, 200, "{}", host.text());
    let giant = json!({"mode": "two_headed_giant", "seats": [0, 1, 1, 0]});
    assert_eq!(info(&server, &game.id).await["house_rules"]["teams"], giant);

    start(&server.registry, &game.id, 4).await;
    let late = put_teams(&server, &game.id, game.host(), &Value::Null).await;
    assert_eq!(late.status, 409, "{}", late.text());
    assert_eq!(info(&server, &game.id).await["house_rules"]["teams"], giant);
}
