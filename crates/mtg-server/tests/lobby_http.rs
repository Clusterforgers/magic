//! Goal: `GET /api/lobbies` over real HTTP lists exactly the public games still in their lobby, newest first and at most fifty, with the host, format, seats filled and total and age; never lists an invite game or one that started; follows joins, leaves and the host's switch; carries an `ETag` of what the rows say (not of their age) and answers `304` to it; turns an address away after thirty requests a minute; and no answer on the way holds a token, a join code or a seat link.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::collections::BTreeSet;
use std::time::Duration;

use mtg_server::registry::{GameOptions, SeatRequest};
use mtg_server::{Visibility, demo};
use net::lobby::{self, Made, bearer, call, listed, lobbies, post_as};
use net::{TestServer, bot_config};
use rstest::rstest;
use serde_json::json;

async fn server() -> TestServer {
    TestServer::start(bot_config()).await
}

/// A server that lets a test ask for the list as often as it likes.
async fn roomy_server() -> TestServer {
    let mut config = bot_config();
    config.auth.limits.lobby_requests_per_ip = 100_000;
    TestServer::start(config).await
}

fn keys(value: &serde_json::Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[tokio::test]
async fn a_public_game_is_listed_with_what_the_browser_needs_and_nothing_else() {
    let server = server().await;
    let made = lobby::make(server.addr, "Ann", 4, 1, "public").await;
    let reply = lobbies(server.addr, &[]).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));
    assert!(
        reply
            .header("content-type")
            .is_some_and(|t| t.starts_with("application/json"))
    );

    let json = reply.json();
    assert_eq!(keys(&json), BTreeSet::from(["games"]));
    let games = json["games"].as_array().unwrap();
    assert_eq!(games.len(), 1);
    let game = &games[0];
    assert_eq!(
        keys(game),
        BTreeSet::from([
            "age_secs",
            "created_at",
            "filled",
            "format",
            "game",
            "host",
            "total"
        ])
    );
    assert_eq!(game["game"], made.id.as_str());
    assert_eq!(game["host"], "Ann");
    assert_eq!(game["format"], "normal");
    assert_eq!(
        (game["filled"].as_u64(), game["total"].as_u64()),
        (Some(1), Some(4))
    );
    assert!(game["age_secs"].as_u64().unwrap() < 60);
    let stamp: String = game["created_at"]
        .as_str()
        .unwrap()
        .chars()
        .map(|c| if c.is_ascii_digit() { '9' } else { c })
        .collect();
    assert_eq!(stamp, "9999-99-99T99:99:99Z");
}

#[tokio::test]
async fn with_no_public_game_the_list_is_empty() {
    let server = server().await;
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        Vec::<String>::new()
    );
    lobby::make(server.addr, "Ann", 3, 1, "invite").await;
    let reply = lobbies(server.addr, &[]).await;
    assert_eq!(reply.json(), json!({ "games": [] }));
}

#[rstest]
#[case::default_visibility(None)]
#[case::explicit_invite(Some("invite"))]
#[tokio::test]
async fn an_invite_game_is_never_listed(#[case] visibility: Option<&str>) {
    let server = server().await;
    let mut request = lobby::body("Ann", 3, 1, "invite");
    if visibility.is_none() {
        request.as_object_mut().unwrap().remove("visibility");
    }
    let reply = call(
        server.addr,
        "POST",
        "/api/games",
        Some(&request.to_string()),
        &[],
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn games_are_listed_newest_first_and_a_full_lobby_stays() {
    let server = server().await;
    let first = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    let second = lobby::make(server.addr, "Bo", 2, 2, "public").await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    let third = lobby::make(server.addr, "Cy", 4, 2, "public").await;
    let reply = lobbies(server.addr, &[]).await;
    assert_eq!(listed(&reply), [third.id, second.id.clone(), first.id]);
    // The game with every seat held is full, and still there.
    let rows = reply.json();
    assert_eq!(
        (
            rows["games"][1]["filled"].as_u64(),
            rows["games"][1]["total"].as_u64()
        ),
        (Some(2), Some(2))
    );
}

#[tokio::test]
async fn at_most_fifty_games_are_listed_the_newest() {
    let server = roomy_server().await;
    let mut made = Vec::new();
    for n in 0..55 {
        let seats = vec![
            SeatRequest {
                name: format!("Host {n}"),
                deck: &demo::all()[0],
            },
            SeatRequest {
                name: String::new(),
                deck: &demo::all()[0],
            },
        ];
        let options = GameOptions {
            visibility: Visibility::Public,
            held: Some(1),
        };
        let created = server
            .registry
            .create_with(&seats, options, None)
            .await
            .unwrap();
        made.push(created.id.to_string());
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let ids = listed(&lobbies(server.addr, &[]).await);
    let newest: Vec<String> = made.iter().rev().take(50).cloned().collect();
    assert_eq!(ids, newest);
}

#[tokio::test]
async fn a_game_that_starts_leaves_the_list() {
    let server = server().await;
    let kept = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let started = lobby::make(server.addr, "Bo", 2, 2, "public").await;
    assert_eq!(listed(&lobbies(server.addr, &[]).await).len(), 2);
    lobby::start(&server.registry, &started.id, 2).await;
    assert_eq!(listed(&lobbies(server.addr, &[]).await), [kept.id]);
}

#[tokio::test]
async fn joins_leaves_and_the_hosts_switch_move_the_list() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 3, 1, "public").await;
    let seats = |reply: &net::Reply| {
        let json = reply.json();
        (
            json["games"][0]["filled"].as_u64().unwrap(),
            json["games"][0]["total"].as_u64().unwrap(),
        )
    };
    assert_eq!(seats(&lobbies(server.addr, &[]).await), (1, 3));

    let join = lobby::join(server.addr, &game.id, &json!({ "name": "Bo" }), &[]).await;
    assert_eq!(join.status, 200, "{}", join.text());
    assert_eq!(seats(&lobbies(server.addr, &[]).await), (2, 3));

    let token = join.json()["token"].as_str().unwrap().to_owned();
    let leave = post_as(
        server.addr,
        &format!("/api/games/{}/leave", game.id),
        &token,
    )
    .await;
    assert_eq!(leave.status, 204, "{}", leave.text());
    assert_eq!(seats(&lobbies(server.addr, &[]).await), (1, 3));

    let hide = call(
        server.addr,
        "PATCH",
        &format!("/api/games/{}", game.id),
        Some(r#"{"visibility":"invite"}"#),
        &[("Authorization", &bearer(game.host()))],
    )
    .await;
    assert_eq!(hide.status, 200, "{}", hide.text());
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        Vec::<String>::new()
    );
    let show = call(
        server.addr,
        "PATCH",
        &format!("/api/games/{}", game.id),
        Some(r#"{"visibility":"public"}"#),
        &[("Authorization", &bearer(game.host()))],
    )
    .await;
    assert_eq!(show.status, 200, "{}", show.text());
    assert_eq!(listed(&lobbies(server.addr, &[]).await), [game.id]);
}

/// Everything a visitor can read about a lobby holds none of its secrets.
#[tokio::test]
async fn no_answer_holds_a_token_a_join_code_or_a_seat_link() {
    let server = roomy_server().await;
    let public: Made = lobby::make(server.addr, "Ann", 3, 2, "public").await;
    let invite: Made = lobby::make(server.addr, "Bo", 3, 2, "invite").await;
    let third = lobby::join(server.addr, &public.id, &json!({ "name": "Cy" }), &[]).await;
    let joiner = third.json()["token"].as_str().unwrap().to_owned();

    let mut secrets: Vec<String> = [&public, &invite]
        .iter()
        .flat_map(|made| made.tokens.iter().cloned().chain([made.code.clone()]))
        .collect();
    secrets.push(joiner);
    // The lower-case and hyphenated spellings of a code are secrets too.
    secrets.extend([
        public.code.to_lowercase(),
        format!("{}-{}", &public.code[..4], &public.code[4..]),
    ]);

    let mut seen = vec![lobbies(server.addr, &[]).await];
    seen.push(lobbies(server.addr, &[("If-None-Match", "\"nothing\"")]).await);
    for id in [&public.id, &invite.id] {
        seen.push(call(server.addr, "GET", &format!("/api/games/{id}"), None, &[]).await);
    }
    // Refusals say nothing either.
    seen.push(lobby::join(server.addr, &invite.id, &json!({ "name": "Di" }), &[]).await);
    seen.push(lobby::join_code(server.addr, "ABCD2345", Some("Di")).await);
    seen.push(
        post_as(
            server.addr,
            &format!("/api/games/{}/leave", invite.id),
            "0".repeat(64).as_str(),
        )
        .await,
    );
    let rows = seen[0].json();
    for reply in &seen {
        let text = format!("{} {:?}", reply.text(), reply.headers);
        for secret in &secrets {
            assert!(
                !text.contains(secret.as_str()),
                "an answer holds {secret}: {text}"
            );
        }
        assert!(
            !text.contains("token=") && !text.contains("#seat"),
            "{text}"
        );
    }
    // The only game listed is the public one, and the only things a row says are its own.
    assert_eq!(rows["games"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn the_etag_is_a_strong_tag_of_the_rows_and_a_matching_request_gets_a_304() {
    let server = roomy_server().await;
    let game = lobby::make(server.addr, "Ann", 3, 1, "public").await;
    let first = lobbies(server.addr, &[]).await;
    assert_eq!(first.status, 200);
    let tag = first.header("etag").expect("an ETag").to_owned();
    assert!(
        tag.len() == 34 && tag.starts_with('"') && tag.ends_with('"') && !tag.starts_with("W/"),
        "{tag}"
    );

    // The same listing again: the same tag, however the client spells it.
    for header in [
        tag.clone(),
        format!("W/{tag}"),
        format!("\"other\", {tag}"),
        format!("{tag} , \"other\""),
        "*".to_owned(),
    ] {
        let again = lobbies(server.addr, &[("If-None-Match", &header)]).await;
        assert_eq!(again.status, 304, "{header}: {}", again.text());
        assert!(again.body.is_empty(), "{header}");
        assert_eq!(again.header("etag"), Some(tag.as_str()));
        assert_eq!(again.header("cache-control"), Some("no-store"));
    }
    for header in ["\"other\"", "", "no quotes", &tag[1..], "W/\"other\""] {
        let again = lobbies(server.addr, &[("If-None-Match", header)]).await;
        assert_eq!(again.status, 200, "{header:?}");
        assert_eq!(again.header("etag"), Some(tag.as_str()));
    }

    // A change to a row changes the tag, and the old one no longer matches.
    let join = lobby::join(server.addr, &game.id, &json!({ "name": "Bo" }), &[]).await;
    assert_eq!(join.status, 200);
    let changed = lobbies(server.addr, &[("If-None-Match", &tag)]).await;
    assert_eq!(changed.status, 200);
    let fresh = changed.header("etag").unwrap().to_owned();
    assert_ne!(fresh, tag);
    assert_eq!(
        lobbies(server.addr, &[("If-None-Match", &fresh)])
            .await
            .status,
        304
    );
}

#[tokio::test]
async fn growing_older_does_not_change_the_tag() {
    let server = roomy_server().await;
    lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let first = lobbies(server.addr, &[]).await;
    let tag = first.header("etag").unwrap().to_owned();
    let age = first.json()["games"][0]["age_secs"].as_u64().unwrap();
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    // The age moved on, the tag did not, so a poll that finds nothing new costs no body.
    let later = lobbies(server.addr, &[]).await;
    assert_eq!(later.header("etag"), Some(tag.as_str()));
    assert!(later.json()["games"][0]["age_secs"].as_u64().unwrap() >= age + 2);
    assert_eq!(
        lobbies(server.addr, &[("If-None-Match", &tag)])
            .await
            .status,
        304
    );
}

#[tokio::test]
async fn an_empty_list_has_a_tag_too_and_it_differs_from_a_full_one() {
    let server = roomy_server().await;
    let empty = lobbies(server.addr, &[]).await;
    let empty_tag = empty.header("etag").unwrap().to_owned();
    assert_eq!(
        lobbies(server.addr, &[("If-None-Match", &empty_tag)])
            .await
            .status,
        304
    );
    lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let full = lobbies(server.addr, &[("If-None-Match", &empty_tag)]).await;
    assert_eq!(full.status, 200);
    assert_ne!(full.header("etag"), Some(empty_tag.as_str()));
}

#[tokio::test]
async fn an_address_gets_thirty_requests_a_minute_and_then_a_429_with_retry_after() {
    let server = server().await;
    let mut tag = None;
    for n in 0..30 {
        let reply = lobbies(server.addr, &[]).await;
        assert_eq!(reply.status, 200, "request {n}");
        tag = reply.header("etag").map(str::to_owned);
    }
    // A request that would be a 304 counts like any other.
    let tag = tag.unwrap();
    let blocked = lobbies(server.addr, &[("If-None-Match", &tag)]).await;
    assert_eq!(blocked.status, 429, "{}", blocked.text());
    assert_eq!(blocked.error_code(), "rate_limited");
    let wait: u64 = blocked
        .header("retry-after")
        .expect("Retry-After")
        .parse()
        .unwrap();
    assert!((1..=60).contains(&wait), "{wait}");
    assert_eq!(blocked.header("cache-control"), Some("no-store"));
    // Nothing else is turned away: the limit is for the list.
    let game = call(server.addr, "GET", "/healthz", None, &[]).await;
    assert_eq!(game.status, 200);
    let made = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    assert_eq!(
        lobby::join(server.addr, &made.id, &json!({ "name": "Bo" }), &[])
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn the_limit_lifts_when_its_window_is_over() {
    let mut config = bot_config();
    config.auth.limits.lobby_requests_per_ip = 2;
    config.auth.limits.lobby_window = Duration::from_secs(2);
    let server = TestServer::start(config).await;
    assert_eq!(lobbies(server.addr, &[]).await.status, 200);
    assert_eq!(lobbies(server.addr, &[]).await.status, 200);
    let blocked = lobbies(server.addr, &[]).await;
    assert_eq!(blocked.status, 429);
    assert!(matches!(blocked.header("retry-after"), Some("1" | "2")));
    tokio::time::sleep(Duration::from_millis(2_300)).await;
    assert_eq!(lobbies(server.addr, &[]).await.status, 200);
}

#[tokio::test]
async fn the_forwarded_address_counts_only_behind_a_trusted_proxy() {
    // Without a trusted proxy the header is ignored: every request is from the socket's peer.
    let mut config = bot_config();
    config.auth.limits.lobby_requests_per_ip = 1;
    let server = TestServer::start(config.clone()).await;
    assert_eq!(
        lobbies(server.addr, &[("X-Forwarded-For", "203.0.113.1")])
            .await
            .status,
        200
    );
    assert_eq!(
        lobbies(server.addr, &[("X-Forwarded-For", "203.0.113.2")])
            .await
            .status,
        429
    );

    // Behind one, each client the proxy saw has a budget of its own.
    config.auth.trusted_proxies = 1;
    let behind = TestServer::start(config).await;
    assert_eq!(
        lobbies(behind.addr, &[("X-Forwarded-For", "203.0.113.1")])
            .await
            .status,
        200
    );
    assert_eq!(
        lobbies(behind.addr, &[("X-Forwarded-For", "203.0.113.2")])
            .await
            .status,
        200
    );
    assert_eq!(
        lobbies(behind.addr, &[("X-Forwarded-For", "203.0.113.1")])
            .await
            .status,
        429
    );
}

#[tokio::test]
async fn the_list_takes_no_method_but_get_and_is_open_to_any_page() {
    let server = roomy_server().await;
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        let reply = call(server.addr, method, "/api/lobbies", None, &[]).await;
        assert_eq!(reply.status, 405, "{method}");
        assert_eq!(reply.error_code(), "method_not_allowed");
    }
    // A foreign page may read the list (it is public) but not act.
    let foreign = [("Origin", "https://evil.example")];
    assert_eq!(lobbies(server.addr, &foreign).await.status, 200);
}

#[tokio::test]
async fn a_game_made_with_open_seats_shows_the_creator_one_token_and_a_code() {
    let server = server().await;
    let reply = call(
        server.addr,
        "POST",
        "/api/games",
        Some(&lobby::body("Ann", 3, 1, "public").to_string()),
        &[],
    )
    .await;
    assert_eq!(reply.status, 201);
    let json = reply.json();
    assert_eq!(json["seats"].as_array().unwrap().len(), 1);
    assert_eq!(json["seats"][0]["seat"], 0);
    assert_eq!(json["code"].as_str().unwrap().len(), 8);

    // The lookup shows the open seats with no name, and the host's with its own.
    let id = json["game"].as_str().unwrap();
    let game = call(server.addr, "GET", &format!("/api/games/{id}"), None, &[])
        .await
        .json();
    assert_eq!(
        game["seats"],
        json!([
            {"seat": 0, "name": "Ann"},
            {"seat": 1, "name": ""},
            {"seat": 2, "name": ""},
        ])
    );
}
