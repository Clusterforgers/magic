//! Goal: the REST surface validates every request (seat count, names, decks, format, unknown fields, content type, body size, hostile JSON), answers with small JSON errors, never leaks a secret, never panics, and enforces the cap on concurrent games.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::collections::BTreeSet;

use mtg_server::join_code::JoinCode;
use mtg_server::{BODY_LIMIT, GAME_BODY_LIMIT, MAX_NAME_CHARS, ServerConfig};
use net::{TestServer, bot_config};
use rstest::rstest;
use serde_json::{Value, json};

fn seat(name: &str, deck: &str) -> Value {
    json!({ "name": name, "deck": deck })
}

fn game_with(seats: &[Value]) -> String {
    json!({ "format": "normal", "seats": seats }).to_string()
}

fn many(count: usize) -> String {
    let seats: Vec<Value> = (0..count)
        .map(|i| seat(&format!("P{i}"), "mono-green"))
        .collect();
    game_with(&seats)
}

/// A Commander game of `count` seats, none with a deck yet.
fn commander(count: usize) -> String {
    let seats: Vec<Value> = (0..count).map(|i| json!({ "name": format!("P{i}") })).collect();
    json!({ "format": "commander", "seats": seats }).to_string()
}

fn keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

async fn server() -> TestServer {
    TestServer::start(bot_config()).await
}

#[rstest]
#[case::two_seats(many(2), 201, "")]
#[case::three_seats(many(3), 201, "")]
#[case::four_seats(many(4), 201, "")]
#[case::eight_seats(many(8), 201, "")]
#[case::twelve_seats(many(12), 201, "")]
#[case::thirteen_seats(many(13), 201, "")]
#[case::twenty_four_seats(many(24), 201, "")]
#[case::sixty_four_seats(many(64), 201, "")]
#[case::one_seat(many(1), 400, "invalid_request")]
#[case::no_seats(many(0), 400, "invalid_request")]
#[case::all_decks(game_with(&[seat("A", "mono-green"), seat("B", "mono-red"), seat("C", "mono-white")]), 201, "")]
#[case::name_is_trimmed(game_with(&[seat("  Ann  ", "mono-red"), seat("Bo", "mono-red")]), 201, "")]
#[case::longest_name(game_with(&[seat(&"n".repeat(MAX_NAME_CHARS), "mono-red"), seat("Bo", "mono-red")]), 201, "")]
#[case::unicode_name(game_with(&[seat("Zoë 日本語", "mono-red"), seat("Bo", "mono-red")]), 201, "")]
#[case::name_too_long(game_with(&[seat(&"n".repeat(MAX_NAME_CHARS + 1), "mono-red"), seat("Bo", "mono-red")]), 400, "invalid_request")]
#[case::empty_name(game_with(&[seat("", "mono-red"), seat("Bo", "mono-red")]), 400, "invalid_request")]
#[case::blank_name(game_with(&[seat(" \t ", "mono-red"), seat("Bo", "mono-red")]), 400, "invalid_request")]
#[case::newline_in_name(game_with(&[seat("A\nB", "mono-red"), seat("Bo", "mono-red")]), 400, "invalid_request")]
#[case::nul_in_name(game_with(&[seat("A\u{0}B", "mono-red"), seat("Bo", "mono-red")]), 400, "invalid_request")]
#[case::bidi_override_in_name(game_with(&[seat("A\u{202e}B", "mono-red"), seat("Bo", "mono-red")]), 400, "invalid_request")]
#[case::unknown_deck(game_with(&[seat("A", "mono-purple"), seat("Bo", "mono-red")]), 400, "unknown_deck")]
#[case::empty_deck(game_with(&[seat("A", ""), seat("Bo", "mono-red")]), 400, "unknown_deck")]
#[case::deck_case_matters(game_with(&[seat("A", "Mono-Green"), seat("Bo", "mono-red")]), 400, "unknown_deck")]
#[case::a_format_nobody_serves(json!({"format":"modern","seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "unsupported_format")]
#[case::commander_seats_choose_their_decks_later(json!({"format":"commander","seats":[{"name":"A"},{"name":"B"}]}).to_string(), 201, "")]
#[case::commander_has_six_seats(commander(6), 201, "")]
#[case::commander_has_a_seventh_seat(commander(7), 201, "")]
#[case::commander_has_twenty_four_seats(commander(24), 201, "")]
#[case::commander_has_sixty_four_seats(commander(64), 201, "")]
#[case::commander_needs_two_seats(json!({"format":"commander","seats":[{"name":"A"}]}).to_string(), 400, "invalid_request")]
#[case::format_case_matters(json!({"format":"Normal","seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "unsupported_format")]
#[case::no_format(json!({"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::no_seats_field(json!({"format":"normal"}).to_string(), 400, "invalid_request")]
#[case::a_public_game(json!({"format":"normal","visibility":"public","seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 201, "")]
#[case::an_invite_game(json!({"format":"normal","visibility":"invite","seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 201, "")]
#[case::unknown_visibility(json!({"format":"normal","visibility":"private","seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::visibility_case_matters(json!({"format":"normal","visibility":"Public","seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::visibility_is_null(json!({"format":"normal","visibility":null,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::visibility_is_a_flag(json!({"format":"normal","visibility":true,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::only_the_creator_holds_a_seat(json!({"format":"normal","held":1,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 201, "")]
#[case::every_seat_held(json!({"format":"normal","held":2,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 201, "")]
#[case::an_open_seat_needs_no_name(json!({"format":"normal","held":1,"seats":[seat("A","mono-red"),{"deck":"mono-red"}]}).to_string(), 201, "")]
#[case::an_open_seat_name_is_never_judged(json!({"format":"normal","held":1,"seats":[seat("A","mono-red"),seat("A\nB","mono-red")]}).to_string(), 201, "")]
#[case::an_open_seat_still_needs_a_deck(json!({"format":"normal","held":1,"seats":[seat("A","mono-red"),seat("B","mono-purple")]}).to_string(), 400, "unknown_deck")]
#[case::a_held_seat_needs_a_name(json!({"format":"normal","held":2,"seats":[seat("A","mono-red"),{"deck":"mono-red"}]}).to_string(), 400, "invalid_request")]
#[case::a_held_seat_name_is_judged(json!({"format":"normal","held":2,"seats":[seat("A","mono-red"),seat("A\nB","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::no_seat_held(json!({"format":"normal","held":0,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::more_held_than_seats(json!({"format":"normal","held":3,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::held_is_negative(json!({"format":"normal","held":-1,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::held_is_a_fraction(json!({"format":"normal","held":1.5,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::held_is_text(json!({"format":"normal","held":"1","seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::client_chosen_seed(json!({"format":"normal","seed":7,"seats":[seat("A","mono-red"),seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::unknown_seat_field(json!({"format":"normal","seats":[{"name":"A","deck":"mono-red","token":"x"},seat("B","mono-red")]}).to_string(), 400, "invalid_request")]
#[case::duplicate_key(r#"{"format":"normal","format":"normal","seats":[{"name":"A","deck":"mono-red"},{"name":"B","deck":"mono-red"}]}"#.to_owned(), 400, "invalid_request")]
#[case::name_is_a_number(r#"{"format":"normal","seats":[{"name":5,"deck":"mono-red"},{"name":"B","deck":"mono-red"}]}"#.to_owned(), 400, "invalid_request")]
#[case::seat_is_a_string(r#"{"format":"normal","seats":["A","B"]}"#.to_owned(), 400, "invalid_request")]
#[case::seats_is_a_string(r#"{"format":"normal","seats":"many"}"#.to_owned(), 400, "invalid_request")]
#[case::huge_number(r#"{"format":"normal","seats":1e999}"#.to_owned(), 400, "invalid_request")]
#[case::empty_body(String::new(), 400, "invalid_request")]
#[case::truncated(r#"{"format":"normal","seats":[{"name":"A""#.to_owned(), 400, "invalid_request")]
#[case::trailing_junk(format!("{}x", many(2)), 400, "invalid_request")]
#[case::byte_order_mark(format!("\u{feff}{}", many(2)), 400, "invalid_request")]
#[case::null(String::from("null"), 400, "invalid_request")]
#[case::array(String::from("[]"), 400, "invalid_request")]
#[case::number(String::from("42"), 400, "invalid_request")]
#[case::string(String::from("\"normal\""), 400, "invalid_request")]
#[case::empty_object(String::from("{}"), 400, "invalid_request")]
#[case::deeply_nested("[".repeat(BODY_LIMIT), 400, "invalid_request")]
#[case::deeply_nested_object("{\"a\":".repeat(BODY_LIMIT / 5), 400, "invalid_request")]
#[case::exactly_the_limit(format!("{}{}", many(2), " ".repeat(GAME_BODY_LIMIT - many(2).len())), 201, "")]
#[case::one_byte_over(format!("{}{}", many(2), " ".repeat(GAME_BODY_LIMIT + 1 - many(2).len())), 413, "payload_too_large")]
#[case::far_over(format!("{}{}", many(2), " ".repeat(40 * GAME_BODY_LIMIT)), 413, "payload_too_large")]
#[tokio::test]
async fn creating_a_game_validates_every_request(
    #[case] body: String,
    #[case] status: u16,
    #[case] error: &str,
) {
    let server = server().await;
    let reply = net::post_json(server.addr, "/api/games", &body).await;
    assert_eq!(reply.status, status, "{}", reply.text());
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    assert_eq!(reply.header("x-content-type-options"), Some("nosniff"));
    let json = reply.json();
    if status == 201 {
        assert_eq!(keys(&json), BTreeSet::from(["code", "game", "seats"]));
        let code = json["code"].as_str().unwrap();
        assert!(
            JoinCode::parse(code).is_some_and(|c| c.expose() == code),
            "{code}"
        );
        let seats = json["seats"].as_array().unwrap();
        let tokens: BTreeSet<&str> = seats.iter().map(|s| s["token"].as_str().unwrap()).collect();
        assert_eq!(tokens.len(), seats.len(), "tokens are distinct");
        for (i, seat) in seats.iter().enumerate() {
            assert_eq!(keys(seat), BTreeSet::from(["seat", "token"]));
            assert_eq!(seat["seat"], i);
            let token = seat["token"].as_str().unwrap();
            assert!(token.len() >= 32 && token.bytes().all(|b| b.is_ascii_hexdigit()));
        }
        assert_eq!(server.registry.len(), 1);
    } else {
        // Errors are small, uniform and say nothing about the server.
        assert_eq!(keys(&json), BTreeSet::from(["error", "message"]));
        assert_eq!(json["error"], error);
        assert!(reply.body.len() < 400, "{}", reply.text());
        assert_eq!(server.registry.len(), 0, "a refused request made a game");
    }
}

#[rstest]
#[case::text(Some("text/plain"), 415)]
#[case::form(Some("application/x-www-form-urlencoded"), 415)]
#[case::almost_json(Some("application/jsonx"), 415)]
#[case::none(None, 415)]
#[case::json(Some("application/json"), 201)]
#[case::json_with_charset(Some("application/json; charset=utf-8"), 201)]
#[case::json_upper_case(Some("APPLICATION/JSON"), 201)]
#[tokio::test]
async fn only_json_bodies_are_read(#[case] content_type: Option<&str>, #[case] status: u16) {
    let server = server().await;
    let headers: Vec<(&str, &str)> = content_type
        .map(|t| ("Content-Type", t))
        .into_iter()
        .collect();
    let body = many(2);
    let reply = net::request(server.addr, "POST", "/api/games", &headers, body.as_bytes()).await;
    assert_eq!(reply.status, status);
    if status == 415 {
        assert_eq!(reply.error_code(), "unsupported_media_type");
        assert_eq!(server.registry.len(), 0);
    }
}

#[tokio::test]
async fn bodies_that_are_not_text_or_that_never_stop_are_refused() {
    let server = server().await;
    let headers = [("Content-Type", "application/json")];
    let invalid_utf8 = [b"{\"format\":\"".as_slice(), &[0xff, 0xfe, 0xfd], b"\"}"].concat();
    let reply = net::request(server.addr, "POST", "/api/games", &headers, &invalid_utf8).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (400, "invalid_request")
    );

    // A chunked body of unknown length is cut off at the limit as well.
    let chunk = " ".repeat(GAME_BODY_LIMIT + 1);
    let raw = format!(
        "POST /api/games HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{chunk}\r\n0\r\n\r\n",
        chunk.len()
    );
    let reply = net::send_raw(server.addr, raw.as_bytes()).await;
    assert_eq!(reply.status, 413, "{}", reply.text());

    // Garbage that is not HTTP gets a 400 from the HTTP layer and no game.
    let reply = net::send_raw(server.addr, b"\x16\x03\x01 definitely not http\r\n\r\n").await;
    assert_eq!(reply.status, 400);
    assert_eq!(server.registry.len(), 0);
}

#[rstest]
#[case("PUT", "/api/games", 405)]
#[case("DELETE", "/api/games", 405)]
#[case("GET", "/api/games", 405)]
#[case("POST", "/api/demo-decks", 405)]
#[case("POST", "/healthz", 405)]
#[case("POST", "/api/games/00000000-0000-0000-0000-000000000000", 405)]
#[case("GET", "/", 404)]
#[case("GET", "/api", 404)]
#[case("GET", "/api/games/", 404)]
#[case("GET", "/api/nothing", 404)]
#[case("GET", "/ws", 404)]
#[case("GET", "/ws/only-one-part", 404)]
#[case("GET", "/healthz/extra", 404)]
#[tokio::test]
async fn unknown_routes_and_methods_get_small_json_errors(
    #[case] method: &str,
    #[case] path: &str,
    #[case] status: u16,
) {
    let server = server().await;
    let reply = net::request(server.addr, method, path, &[], b"").await;
    assert_eq!(reply.status, status);
    let code = if status == 404 {
        "not_found"
    } else {
        "method_not_allowed"
    };
    assert_eq!(reply.error_code(), code);
    assert!(reply.body.len() < 200);
}

#[tokio::test]
async fn health_decks_and_game_lookup() {
    let server = server().await;
    let health = net::get(server.addr, "/healthz").await;
    assert_eq!((health.status, health.text().as_str()), (200, "ok"));

    let decks = net::get(server.addr, "/api/demo-decks").await;
    assert_eq!(decks.status, 200);
    let decks = decks.json();
    let decks = decks.as_array().unwrap();
    assert!(decks.len() >= 2);
    let ids: BTreeSet<&str> = decks.iter().map(|d| d["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), decks.len());
    for deck in decks {
        assert_eq!(
            keys(deck),
            BTreeSet::from(["cards", "color", "id", "name", "size"])
        );
        assert_eq!(deck["size"], 60);
        let total: u64 = deck["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["count"].as_u64().unwrap())
            .sum();
        assert_eq!(total, 60);
    }

    let body = game_with(&[
        seat("  Ann ", "mono-green"),
        seat("Bo", ids.iter().next().unwrap()),
    ]);
    let created = net::post_json(server.addr, "/api/games", &body)
        .await
        .json();
    let game = created["game"].as_str().unwrap();
    let reply = net::get(server.addr, &format!("/api/games/{game}")).await;
    assert_eq!(reply.status, 200);
    let view = reply.json();
    assert_eq!(
        keys(&view),
        BTreeSet::from([
            "connected",
            "format",
            "manual_play",
            "ready",
            "result",
            "seats",
            "started",
            "visibility"
        ])
    );
    assert_eq!(view["format"], "normal");
    assert_eq!(view["started"], false);
    assert_eq!(view["manual_play"], false, "built-in decks run whole");
    assert_eq!(
        view["visibility"], "invite",
        "games are invite-only unless asked"
    );
    assert_eq!(view["connected"], json!([]));
    assert_eq!(
        view["ready"],
        json!([]),
        "nobody has pressed Ready in a new game"
    );
    assert_eq!(view["result"], Value::Null);
    assert_eq!(
        view["seats"],
        json!([{"seat": 0, "name": "Ann"}, {"seat": 1, "name": "Bo"}])
    );
    // Nothing secret is in the public view.
    for seat in created["seats"].as_array().unwrap() {
        assert!(!reply.text().contains(seat["token"].as_str().unwrap()));
    }

    for path in [
        "/api/games/nope",
        "/api/games/11111111-2222-4333-8444-555555555555",
        "/api/games/%00",
        "/api/games/%ff",
    ] {
        let reply = net::get(server.addr, path).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (404, "not_found"),
            "{path}"
        );
    }
}

#[tokio::test]
async fn only_allowed_origins_get_cors_headers() {
    let config = ServerConfig {
        allowed_origins: vec!["http://localhost:5173".into()],
        ..bot_config()
    };
    let server = TestServer::start(config).await;
    let preflight = |origin: &'static str| {
        let headers = vec![
            ("Origin", origin),
            ("Access-Control-Request-Method", "POST"),
            ("Access-Control-Request-Headers", "content-type"),
        ];
        let addr = server.addr;
        async move { net::request(addr, "OPTIONS", "/api/games", &headers, b"").await }
    };
    let allowed = preflight("http://localhost:5173").await;
    assert_eq!(
        allowed.header("access-control-allow-origin"),
        Some("http://localhost:5173")
    );
    assert!(
        allowed
            .header("access-control-allow-methods")
            .unwrap()
            .contains("POST")
    );
    let stranger = preflight("https://evil.example").await;
    assert_eq!(stranger.header("access-control-allow-origin"), None);

    let headers = [
        ("Content-Type", "application/json"),
        ("Origin", "http://localhost:5173"),
    ];
    let reply = net::request(
        server.addr,
        "POST",
        "/api/games",
        &headers,
        many(2).as_bytes(),
    )
    .await;
    assert_eq!(reply.status, 201);
    assert_eq!(
        reply.header("access-control-allow-origin"),
        Some("http://localhost:5173")
    );
}

#[tokio::test]
async fn the_cap_on_concurrent_games_holds() {
    let config = ServerConfig {
        max_games: 3,
        ..bot_config()
    };
    let server = TestServer::start(config).await;
    let mut games = Vec::new();
    for _ in 0..3 {
        games.push(
            net::create_game(server.addr, serde_json::from_str(&many(2)).unwrap())
                .await
                .0,
        );
    }
    for _ in 0..3 {
        let reply = net::post_json(server.addr, "/api/games", &many(2)).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (503, "server_full")
        );
    }
    assert_eq!(server.registry.len(), 3);
    // The games that exist are unaffected, and so is everything that is not a creation.
    for game in &games {
        assert_eq!(
            net::get(server.addr, &format!("/api/games/{game}"))
                .await
                .status,
            200
        );
    }
    assert_eq!(net::get(server.addr, "/healthz").await.status, 200);
}
