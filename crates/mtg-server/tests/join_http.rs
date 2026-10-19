//! Goal: joining over real HTTP: a guest takes the first open seat of a public lobby by the game's id, or of any game by its join code however it is typed, under a name of 1 to 32 characters, and is given that seat's token, which opens the seat's socket; an unknown id, a malformed id, an invite game reached by its id (started or not) and a wrong code are one byte-identical 404; a full or a started game says so; wrong codes are counted per address, five a minute, and then even the right code is turned away with Retry-After until the minute is over; bodies that are not a valid request are refused before any seat or code is looked at.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::collections::BTreeSet;
use std::time::Duration;

use net::lobby::{self, call, join, join_code};
use net::{Reply, TestServer, bot_config};
use rstest::rstest;
use serde_json::{Value, json};

async fn server() -> TestServer {
    TestServer::start(bot_config()).await
}

/// A server that does not count wrong codes against anybody.
async fn forgiving_server() -> TestServer {
    let mut config = bot_config();
    config.auth.limits.code_misses_per_ip = 100_000;
    TestServer::start(config).await
}

fn keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

/// The names at the seats of `game`, in seat order.
async fn names(server: &TestServer, game: &str) -> Vec<String> {
    let reply = call(server.addr, "GET", &format!("/api/games/{game}"), None, &[]).await;
    reply.json()["seats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|seat| seat["name"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_guest_joins_a_public_game_by_its_id_and_the_token_opens_the_seat() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 3, 1, "public").await;

    let first = join(server.addr, &game.id, &json!({ "name": "Bo" }), &[]).await;
    assert_eq!(first.status, 200, "{}", first.text());
    assert_eq!(first.header("cache-control"), Some("no-store"));
    let body = first.json();
    assert_eq!(keys(&body), BTreeSet::from(["seat", "token"]));
    assert_eq!(body["seat"], 1);
    let token = body["token"].as_str().unwrap().to_owned();
    assert!(token.len() >= 32 && token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bo", ""]);

    let second = join(server.addr, &game.id, &json!({ "name": "Cy" }), &[]).await;
    assert_eq!(second.json()["seat"], 2);
    assert_ne!(second.json()["token"], body["token"]);
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bo", "Cy"]);

    // The token is the seat's credential: it opens that seat's socket and no other.
    let mut socket = net::connect(server.addr, &game.id, &token).await;
    assert_eq!(socket.welcome(&game.id).await.index(), 1);
}

#[tokio::test]
async fn a_guest_may_take_several_seats_of_one_game() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 3, 1, "public").await;
    for expected in [1, 2] {
        let reply = join(server.addr, &game.id, &json!({ "name": "Bo" }), &[]).await;
        assert_eq!(reply.json()["seat"], expected);
    }
    // Both seats are Bo's and each has its own token.
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bo", "Bo"]);
}

#[rstest]
#[case::as_shown(|code: &str| code.to_owned())]
#[case::lower_case(|code: &str| code.to_lowercase())]
#[case::hyphenated(|code: &str| format!("{}-{}", &code[..4], &code[4..]))]
#[case::spaced_and_padded(|code: &str| format!("  {} {}\n", &code[..4], &code[4..]))]
#[tokio::test]
async fn a_code_joins_any_game_however_it_is_typed(#[case] typed: fn(&str) -> String) {
    let server = server().await;
    for visibility in ["public", "invite"] {
        let game = lobby::make(server.addr, "Ann", 2, 1, visibility).await;
        let reply = join_code(server.addr, &typed(&game.code), Some("Bo")).await;
        assert_eq!(reply.status, 200, "{visibility}: {}", reply.text());
        assert_eq!(reply.json()["seat"], 1);
        assert!(reply.json()["token"].is_string());
        assert_eq!(names(&server, &game.id).await, ["Ann", "Bo"]);
    }
}

#[tokio::test]
async fn an_invite_game_is_reached_by_its_code_and_not_by_its_id() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "invite").await;
    let by_id = join(server.addr, &game.id, &json!({ "name": "Bo" }), &[]).await;
    assert_eq!(by_id.status, 404);
    assert_eq!(
        names(&server, &game.id).await,
        ["Ann", ""],
        "nothing was taken"
    );
    let by_code = join_code(server.addr, &game.code, Some("Bo")).await;
    assert_eq!(by_code.status, 200);

    // The host can list the game and then its id is enough.
    let other = lobby::make(server.addr, "Cy", 2, 1, "public").await;
    assert_eq!(
        join(server.addr, &other.id, &json!({ "name": "Di" }), &[])
            .await
            .status,
        200
    );
}

/// The one answer for everything a visitor must not be able to tell apart.
#[tokio::test]
async fn an_unknown_game_an_invite_game_and_a_wrong_code_are_one_404() {
    let server = forgiving_server().await;
    let open_invite = lobby::make(server.addr, "Ann", 3, 1, "invite").await;
    let full_invite = lobby::make(server.addr, "Bo", 2, 2, "invite").await;
    let started_invite = lobby::make(server.addr, "Cy", 2, 2, "invite").await;
    lobby::start(&server.registry, &started_invite.id, 2).await;
    let guest = json!({ "name": "Zed" });

    let mut refusals: Vec<(String, Reply)> = Vec::new();
    for (what, id) in [
        ("an unknown id", "6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55"),
        ("a malformed id", "not-a-game"),
        ("an invite game with an open seat", open_invite.id.as_str()),
        (
            "an invite game with every seat held",
            full_invite.id.as_str(),
        ),
        ("a started invite game", started_invite.id.as_str()),
    ] {
        refusals.push((what.to_owned(), join(server.addr, id, &guest, &[]).await));
    }
    for (what, code) in [
        ("a wrong code", "ABCD2345"),
        ("a malformed code", "not a code"),
        ("an empty code", ""),
        ("a game id as a code", open_invite.id.as_str()),
    ] {
        refusals.push((
            what.to_owned(),
            join_code(server.addr, code, Some("Zed")).await,
        ));
    }

    let (first_what, first) = &refusals[0];
    assert_eq!(first.status, 404, "{first_what}: {}", first.text());
    assert_eq!(
        first.json(),
        json!({"error": "not_found", "message": "no such resource"})
    );
    for (what, reply) in &refusals {
        assert_eq!(reply.status, 404, "{what}: {}", reply.text());
        assert_eq!(reply.body, first.body, "{what} differs from {first_what}");
        assert_eq!(
            reply.header("content-type"),
            first.header("content-type"),
            "{what}"
        );
        assert_eq!(
            reply.headers_named("set-cookie"),
            Vec::<&str>::new(),
            "{what}"
        );
        assert_eq!(reply.header("retry-after"), None, "{what}");
    }
    // None of it took a seat.
    assert_eq!(names(&server, &open_invite.id).await, ["Ann", "", ""]);
}

#[tokio::test]
async fn a_full_game_and_a_started_game_say_so() {
    let server = forgiving_server().await;
    let guest = json!({ "name": "Zed" });
    let full_public = lobby::make(server.addr, "Ann", 2, 2, "public").await;
    let full_invite = lobby::make(server.addr, "Bo", 2, 2, "invite").await;
    let started_public = lobby::make(server.addr, "Cy", 2, 2, "public").await;
    let started_invite = lobby::make(server.addr, "Di", 2, 2, "invite").await;
    lobby::start(&server.registry, &started_public.id, 2).await;
    lobby::start(&server.registry, &started_invite.id, 2).await;

    let full = join(server.addr, &full_public.id, &guest, &[]).await;
    assert_eq!((full.status, full.error_code().as_str()), (409, "full"));
    // A game that started was full, and says that it started.
    let started = join(server.addr, &started_public.id, &guest, &[]).await;
    assert_eq!(
        (started.status, started.error_code().as_str()),
        (409, "started")
    );

    for (game, expected) in [
        (&full_public, "full"),
        (&full_invite, "full"),
        (&started_public, "started"),
        (&started_invite, "started"),
    ] {
        let reply = join_code(server.addr, &game.code, Some("Zed")).await;
        assert_eq!(reply.status, 409, "{expected}: {}", reply.text());
        assert_eq!(reply.error_code(), expected);
    }
}

#[tokio::test]
async fn a_game_fills_up_and_the_next_visitor_is_told_so() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 3, 1, "public").await;
    for name in ["Bo", "Cy"] {
        let reply = join(server.addr, &game.id, &json!({ "name": name }), &[]).await;
        assert_eq!(reply.status, 200);
    }
    let late = join(server.addr, &game.id, &json!({ "name": "Di" }), &[]).await;
    assert_eq!((late.status, late.error_code().as_str()), (409, "full"));
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bo", "Cy"]);
}

#[rstest]
#[case::the_shortest(json!({"name": "B"}), Some("B"))]
#[case::the_longest(json!({"name": "n".repeat(32)}), Some("nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn"))]
#[case::trimmed(json!({"name": "  Bo \t"}), Some("Bo"))]
#[case::unicode(json!({"name": "Zoë 日本語"}), Some("Zoë 日本語"))]
#[case::no_name(json!({}), None)]
#[case::null_name(json!({"name": null}), None)]
#[case::empty(json!({"name": ""}), None)]
#[case::blank(json!({"name": " \t "}), None)]
#[case::one_too_long(json!({"name": "n".repeat(33)}), None)]
#[case::newline(json!({"name": "A\nB"}), None)]
#[case::nul(json!({"name": "A\u{0}B"}), None)]
#[case::bidi_override(json!({"name": "A\u{202e}B"}), None)]
#[case::zero_width(json!({"name": "A\u{200b}B"}), None)]
#[tokio::test]
async fn a_guest_needs_a_name_of_one_to_32_characters(
    #[case] body: Value,
    #[case] seat: Option<&str>,
) {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let by_id = join(server.addr, &game.id, &body, &[]).await;
    let other = lobby::make(server.addr, "Cy", 2, 1, "public").await;
    let mut with_code = body.clone();
    with_code["code"] = json!(other.code);
    let by_code = call(
        server.addr,
        "POST",
        "/api/join",
        Some(&with_code.to_string()),
        &[],
    )
    .await;
    for (reply, id) in [(&by_id, &game.id), (&by_code, &other.id)] {
        if let Some(name) = seat {
            assert_eq!(reply.status, 200, "{}", reply.text());
            assert_eq!(names(&server, id).await[1], name);
        } else {
            assert_eq!(reply.status, 400, "{}", reply.text());
            assert_eq!(reply.error_code(), "invalid_name");
            assert_eq!(names(&server, id).await[1], "", "a seat was taken");
        }
    }
}

#[tokio::test]
async fn the_answer_to_a_missing_name_does_not_depend_on_the_game() {
    let server = forgiving_server().await;
    let invite = lobby::make(server.addr, "Ann", 2, 1, "invite").await;
    let unknown = "6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55";
    for id in [invite.id.as_str(), unknown] {
        let reply = join(server.addr, id, &json!({}), &[]).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (400, "invalid_name"),
            "{id}"
        );
    }
    // Nor does it tell a right code from a wrong one, or cost the address a miss.
    for code in [invite.code.as_str(), "ABCD2345"] {
        let reply = join_code(server.addr, code, None).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (400, "invalid_name"),
            "{code}"
        );
    }
}

#[rstest]
#[case::not_json_at_all("{")]
#[case::a_number("7")]
#[case::empty("")]
#[case::unknown_field(r#"{"name":"Bo","seat":1}"#)]
#[case::a_token_field(r#"{"name":"Bo","token":"x"}"#)]
#[case::name_is_a_number(r#"{"name":7}"#)]
#[case::duplicate_key(r#"{"name":"Bo","name":"Cy"}"#)]
#[tokio::test]
async fn a_body_that_is_not_a_join_request_is_refused_before_anything_is_looked_up(
    #[case] body: &str,
) {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let path = format!("/api/games/{}/join", game.id);
    let reply = call(server.addr, "POST", &path, Some(body), &[]).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (400, "invalid_request"),
        "{}",
        reply.text()
    );
    // The same for an unknown game: the body is judged first.
    let reply = call(
        server.addr,
        "POST",
        "/api/games/6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55/join",
        Some(body),
        &[],
    )
    .await;
    assert_eq!(reply.status, 400);
    assert_eq!(names(&server, &game.id).await, ["Ann", ""]);
}

#[rstest]
#[case::no_code(r#"{"name":"Bo"}"#)]
#[case::code_is_a_number(r#"{"code":12345678,"name":"Bo"}"#)]
#[case::code_is_null(r#"{"code":null,"name":"Bo"}"#)]
#[case::unknown_field(r#"{"code":"ABCD2345","name":"Bo","game":"x"}"#)]
#[case::not_json("code=ABCD2345")]
#[tokio::test]
async fn a_body_that_is_not_a_code_request_is_refused_and_costs_no_miss(#[case] body: &str) {
    let mut config = bot_config();
    config.auth.limits.code_misses_per_ip = 1;
    let server = TestServer::start(config).await;
    for _ in 0..3 {
        let reply = call(server.addr, "POST", "/api/join", Some(body), &[]).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (400, "invalid_request"),
            "{}",
            reply.text()
        );
    }
    // The one miss the address has is still there.
    assert_eq!(
        join_code(server.addr, "ABCD2345", Some("Bo")).await.status,
        404
    );
}

#[rstest]
#[case::text("text/plain", 415)]
#[case::form("application/x-www-form-urlencoded", 415)]
#[case::json("application/json", 200)]
#[case::json_with_charset("application/json; charset=utf-8", 200)]
#[tokio::test]
async fn only_json_bodies_are_read(#[case] content_type: &str, #[case] status: u16) {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let path = format!("/api/games/{}/join", game.id);
    let reply = net::request(
        server.addr,
        "POST",
        &path,
        &[("Content-Type", content_type)],
        br#"{"name":"Bo"}"#,
    )
    .await;
    assert_eq!(reply.status, status, "{}", reply.text());
    let bare = net::request(server.addr, "POST", &path, &[], br#"{"name":"Cy"}"#).await;
    assert_eq!(
        (bare.status, bare.error_code().as_str()),
        (415, "unsupported_media_type")
    );
}

#[tokio::test]
async fn a_body_over_the_limit_is_refused() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let huge = json!({ "name": "n".repeat(mtg_server::BODY_LIMIT) });
    let reply = join(server.addr, &game.id, &huge, &[]).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (413, "payload_too_large")
    );
    let reply = join_code(server.addr, &"A".repeat(mtg_server::BODY_LIMIT), Some("Bo")).await;
    assert_eq!(reply.status, 413);
}

#[tokio::test]
async fn five_wrong_codes_a_minute_then_even_the_right_code_is_turned_away() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 12, 1, "invite").await;
    let wrong = |n: usize| format!("AAAAAAA{n}");
    // Hits cost nothing, however many: three misses, a hit, two more misses are five misses.
    let mut outcomes = Vec::new();
    for attempt in ["miss", "miss", "miss", "hit", "miss", "miss"] {
        let code = if attempt == "hit" {
            game.code.clone()
        } else {
            wrong(2)
        };
        outcomes.push(join_code(server.addr, &code, Some("Bo")).await.status);
    }
    assert_eq!(outcomes, [404, 404, 404, 200, 404, 404]);

    let blocked = join_code(server.addr, &game.code, Some("Bo")).await;
    assert_eq!(blocked.status, 429, "{}", blocked.text());
    assert_eq!(blocked.error_code(), "too_many_attempts");
    let wait: u64 = blocked
        .header("retry-after")
        .expect("Retry-After")
        .parse()
        .unwrap();
    assert!((1..=60).contains(&wait), "{wait}");
    assert_eq!(blocked.header("cache-control"), Some("no-store"));
    // Still blocked: a try that is turned away is not a new miss, and the answer does not change.
    let again = join_code(server.addr, "ZZZZZZZ2", Some("Bo")).await;
    assert_eq!(
        (again.status, again.error_code().as_str()),
        (429, "too_many_attempts")
    );
    // Joining by id is a different door and is not counted.
    let other = lobby::make(server.addr, "Cy", 2, 1, "public").await;
    assert_eq!(
        join(server.addr, &other.id, &json!({ "name": "Di" }), &[])
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn the_block_lifts_when_the_window_is_over() {
    let mut config = bot_config();
    config.auth.limits.code_misses_per_ip = 1;
    config.auth.limits.code_window = Duration::from_secs(2);
    let server = TestServer::start(config).await;
    let game = lobby::make(server.addr, "Ann", 3, 1, "invite").await;
    assert_eq!(
        join_code(server.addr, "ABCD2345", Some("Bo")).await.status,
        404
    );
    let blocked = join_code(server.addr, &game.code, Some("Bo")).await;
    assert_eq!(blocked.status, 429);
    assert!(matches!(blocked.header("retry-after"), Some("1" | "2")));
    tokio::time::sleep(Duration::from_millis(2_300)).await;
    assert_eq!(
        join_code(server.addr, &game.code, Some("Bo")).await.status,
        200
    );
}

#[tokio::test]
async fn every_address_has_a_budget_of_its_own() {
    let mut config = bot_config();
    config.auth.limits.code_misses_per_ip = 2;
    config.auth.trusted_proxies = 1;
    let server = TestServer::start(config).await;
    let game = lobby::make(server.addr, "Ann", 4, 1, "invite").await;
    let right = game.code.as_str();
    let wrong = "ABCD2345";
    assert_eq!(code_from(&server, "203.0.113.1", wrong).await, 404);
    assert_eq!(code_from(&server, "203.0.113.1", wrong).await, 404);
    assert_eq!(code_from(&server, "203.0.113.1", wrong).await, 429);
    assert_eq!(code_from(&server, "203.0.113.1", right).await, 429);
    // Another client has its own two misses, and a network of IPv6 addresses is one client.
    assert_eq!(code_from(&server, "203.0.113.2", right).await, 200);
    assert_eq!(code_from(&server, "2001:db8:1:2::1", wrong).await, 404);
    assert_eq!(code_from(&server, "2001:db8:1:2:ffff::9", wrong).await, 404);
    assert_eq!(code_from(&server, "2001:db8:1:2::77", wrong).await, 429);
    assert_eq!(code_from(&server, "2001:db8:1:3::1", wrong).await, 404);
}

/// The status of a join by `code` that a proxy says came from `address`.
async fn code_from(server: &TestServer, address: &str, code: &str) -> u16 {
    let body = json!({ "code": code, "name": "Bo" }).to_string();
    let headers = [("X-Forwarded-For", address)];
    call(server.addr, "POST", "/api/join", Some(&body), &headers)
        .await
        .status
}

#[tokio::test]
async fn on_a_server_without_a_database_a_session_cookie_means_nothing() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let cookie = [("Cookie", "mtg_session=0123456789abcdef")];
    // The visitor is a guest, so a name is needed, and with one the join works.
    let nameless = join(server.addr, &game.id, &json!({}), &cookie).await;
    assert_eq!(
        (nameless.status, nameless.error_code().as_str()),
        (400, "invalid_name")
    );
    let named = join(server.addr, &game.id, &json!({ "name": "Bo" }), &cookie).await;
    assert_eq!(named.status, 200, "{}", named.text());
    assert!(named.json()["token"].is_string());
}
