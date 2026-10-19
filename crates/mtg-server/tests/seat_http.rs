//! Goal: giving a seat back, removing a player and listing or hiding the game over real HTTP: the holder of a claimed seat leaves with its token, only the host removes a player or switches the visibility (which the lookup of the game then reports), a stranger learns nothing (one 404 whatever is wrong), a seat made with the game cannot be given back, after the start every change answers 409 started, the old token dies with the seat, and a foreign origin is refused on every call that changes something while a listed origin may use the new methods and headers.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use net::lobby::{self, Made, bearer, call, join, listed, lobbies, post_as};
use net::{Reply, TestServer, bot_config};
use rstest::rstest;
use serde_json::{Value, json};

async fn server() -> TestServer {
    TestServer::start(bot_config()).await
}

/// A server for tests that ask for the list many times.
async fn roomy_server() -> TestServer {
    let mut config = bot_config();
    config.auth.limits.lobby_requests_per_ip = 100_000;
    config.auth.limits.code_misses_per_ip = 100_000;
    TestServer::start(config).await
}

/// A public game of four seats whose host holds seat 0, and two guests who joined seats 1 and 2.
struct Table {
    game: Made,
    bo: String,
    cy: String,
}

async fn table(server: &TestServer) -> Table {
    let game = lobby::make(server.addr, "Ann", 4, 1, "public").await;
    let mut tokens = Vec::new();
    for name in ["Bo", "Cy"] {
        let reply = join(server.addr, &game.id, &json!({ "name": name }), &[]).await;
        assert_eq!(reply.status, 200, "{}", reply.text());
        tokens.push(reply.json()["token"].as_str().unwrap().to_owned());
    }
    Table {
        game,
        bo: tokens[0].clone(),
        cy: tokens[1].clone(),
    }
}

async fn names(server: &TestServer, game: &str) -> Vec<String> {
    let reply = call(server.addr, "GET", &format!("/api/games/{game}"), None, &[]).await;
    reply.json()["seats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|seat| seat["name"].as_str().unwrap().to_owned())
        .collect()
}

/// What `GET /api/games/{id}` says about who may find the game.
async fn shown_visibility(server: &TestServer, game: &str) -> Value {
    let reply = call(server.addr, "GET", &format!("/api/games/{game}"), None, &[]).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["visibility"].clone()
}

fn leave_path(game: &str) -> String {
    format!("/api/games/{game}/leave")
}

fn release_path(game: &str, seat: &str) -> String {
    format!("/api/games/{game}/seats/{seat}/release")
}

fn game_path(game: &str) -> String {
    format!("/api/games/{game}")
}

async fn patch(server: &TestServer, game: &str, token: &str, body: &str) -> Reply {
    call(
        server.addr,
        "PATCH",
        &game_path(game),
        Some(body),
        &[("Authorization", &bearer(token))],
    )
    .await
}

fn is_empty_204(reply: &Reply) -> bool {
    reply.status == 204 && reply.body.is_empty()
}

#[tokio::test]
async fn a_holder_gives_the_seat_back_and_its_token_dies_with_it() {
    let server = server().await;
    let table = table(&server).await;
    let id = &table.game.id;

    let left = post_as(server.addr, &leave_path(id), &table.bo).await;
    assert!(is_empty_204(&left), "{} {}", left.status, left.text());
    assert_eq!(left.header("cache-control"), Some("no-store"));
    assert_eq!(names(&server, id).await, ["Ann", "", "Cy", ""]);

    // The token no longer opens anything: not the REST calls, not the socket.
    let again = post_as(server.addr, &leave_path(id), &table.bo).await;
    assert_eq!(
        (again.status, again.error_code().as_str()),
        (404, "not_found")
    );
    let (status, _) = net::refused(server.addr, id, &table.bo, None).await;
    assert_eq!(status, 404);

    // The seat is free for the next visitor, who gets a new token.
    let next = join(server.addr, id, &json!({ "name": "Di" }), &[]).await;
    assert_eq!(next.json()["seat"], 1);
    assert_ne!(next.json()["token"], json!(table.bo));
    assert_eq!(names(&server, id).await, ["Ann", "Di", "Cy", ""]);
    // The old token still opens nothing, now that someone else sits there.
    assert_eq!(
        post_as(server.addr, &leave_path(id), &table.bo)
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn a_seat_made_with_the_game_cannot_be_given_back() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 3, 2, "public").await;
    // The host's own seat and a seat whose link was handed out are not seats a join took.
    for token in &game.tokens {
        let reply = post_as(server.addr, &leave_path(&game.id), token).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (409, "not_claimed")
        );
    }
    let release = post_as(server.addr, &release_path(&game.id, "1"), game.host()).await;
    assert_eq!(
        (release.status, release.error_code().as_str()),
        (409, "not_claimed")
    );
    assert_eq!(names(&server, &game.id).await, ["Ann", "Player 1", ""]);
    // The tokens still work.
    let mut socket = net::connect(server.addr, &game.id, &game.tokens[1]).await;
    assert_eq!(socket.welcome(&game.id).await.index(), 1);
}

/// What a request with no standing at the table is told, whatever it asks for.
#[tokio::test]
async fn a_stranger_learns_nothing_whatever_is_asked() {
    let server = roomy_server().await;
    let table = table(&server).await;
    let other = lobby::make(server.addr, "Zed", 2, 2, "invite").await;
    let id = table.game.id.as_str();
    let unknown = "6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55";
    let wrong_token = "0".repeat(64);

    let asks = |game: &str| {
        [
            ("POST", leave_path(game), None),
            ("POST", release_path(game, "1"), None),
            ("PATCH", game_path(game), Some(r#"{"visibility":"invite"}"#)),
        ]
    };
    // A wrong token, the token of a seat in another game and a malformed token, for a real game
    // and for one that does not exist: one and the same answer.
    let mut answers: Vec<(String, Reply)> = Vec::new();
    for (what, token) in [
        ("a wrong token", wrong_token.as_str()),
        ("a token of another game", other.host()),
        ("a malformed token", "not a token"),
    ] {
        for game in [id, unknown, "not-a-game"] {
            for (method, path, body) in asks(game) {
                let reply = call(
                    server.addr,
                    method,
                    &path,
                    body,
                    &[("Authorization", &bearer(token))],
                )
                .await;
                answers.push((format!("{what}: {method} {path}"), reply));
            }
        }
    }
    let (_, first) = &answers[0];
    assert_eq!(first.status, 404, "{}", first.text());
    assert_eq!(
        first.json(),
        json!({"error": "not_found", "message": "no such resource"})
    );
    for (what, reply) in &answers {
        assert_eq!(reply.status, 404, "{what}: {}", reply.text());
        assert_eq!(reply.body, first.body, "{what}");
    }
    // Nothing changed.
    assert_eq!(names(&server, id).await, ["Ann", "Bo", "Cy", ""]);
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        std::slice::from_ref(&table.game.id)
    );
}

#[tokio::test]
async fn without_a_token_a_server_with_no_accounts_says_so_like_the_other_session_routes() {
    let server = server().await;
    let table = table(&server).await;
    let id = table.game.id.as_str();
    for (method, path, body) in [
        ("POST", leave_path(id), None),
        ("POST", release_path(id, "1"), None),
        ("PATCH", game_path(id), Some(r#"{"visibility":"invite"}"#)),
    ] {
        // No credential at all, a cookie nobody can honour, a scheme that is not a seat token and a
        // bearer scheme with no token after it.
        for headers in [
            vec![],
            vec![("Cookie", "mtg_session=0123456789abcdef")],
            vec![("Authorization", "Basic dXNlcjpwYXNz")],
            vec![("Authorization", "Bearer")],
        ] {
            let reply = call(server.addr, method, &path, body, &headers).await;
            assert_eq!(
                reply.status,
                503,
                "{method} {path} {headers:?}: {}",
                reply.text()
            );
            assert_eq!(reply.error_code(), "accounts_unavailable");
        }
    }
    // The same server still lets a token do all of it.
    let leave = post_as(server.addr, &leave_path(id), &table.bo).await;
    assert_eq!(leave.status, 204);
}

#[tokio::test]
async fn a_bearer_scheme_is_matched_without_regard_to_case() {
    let server = server().await;
    let table = table(&server).await;
    let header = format!("bEaReR {}", table.bo);
    let reply = call(
        server.addr,
        "POST",
        &leave_path(&table.game.id),
        None,
        &[("Authorization", &header)],
    )
    .await;
    assert_eq!(reply.status, 204, "{}", reply.text());
}

#[tokio::test]
async fn the_host_removes_a_player_and_that_players_token_dies() {
    let server = server().await;
    let table = table(&server).await;
    let id = &table.game.id;
    let removed = post_as(server.addr, &release_path(id, "1"), table.game.host()).await;
    assert!(
        is_empty_204(&removed),
        "{} {}",
        removed.status,
        removed.text()
    );
    assert_eq!(names(&server, id).await, ["Ann", "", "Cy", ""]);
    assert_eq!(
        post_as(server.addr, &leave_path(id), &table.bo)
            .await
            .status,
        404
    );
    assert_eq!(net::refused(server.addr, id, &table.bo, None).await.0, 404);
    // Cy keeps the seat.
    let mut socket = net::connect(server.addr, id, &table.cy).await;
    assert_eq!(socket.welcome(id).await.index(), 2);
}

#[tokio::test]
async fn only_the_host_removes_a_player() {
    let server = server().await;
    let table = table(&server).await;
    let id = &table.game.id;
    // A player is the holder of their own seat and still may not use the host's route, on their own
    // seat or on anyone else's.
    for (token, seat) in [
        (&table.bo, "1"),
        (&table.bo, "2"),
        (&table.cy, "1"),
        (&table.cy, "2"),
    ] {
        let reply = post_as(server.addr, &release_path(id, seat), token).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (403, "forbidden"),
            "seat {seat}"
        );
    }
    assert_eq!(names(&server, id).await, ["Ann", "Bo", "Cy", ""]);
}

#[rstest]
#[case::an_open_seat("3", 409, "not_claimed")]
#[case::the_hosts_own_seat("0", 409, "not_claimed")]
#[case::a_seat_no_game_has("12", 404, "not_found")]
#[case::far_beyond("99", 404, "not_found")]
#[case::three_digits("001", 404, "not_found")]
#[case::leading_zero("01", 404, "not_found")]
#[case::negative("-1", 404, "not_found")]
#[case::plus_sign("+1", 404, "not_found")]
#[case::text("one", 404, "not_found")]
#[case::empty("%20", 404, "not_found")]
#[case::a_fraction("1.5", 404, "not_found")]
#[tokio::test]
async fn the_host_names_a_seat_that_a_join_took_or_is_told_why_not(
    #[case] seat: &str,
    #[case] status: u16,
    #[case] code: &str,
) {
    let server = server().await;
    let table = table(&server).await;
    let reply = post_as(
        server.addr,
        &release_path(&table.game.id, seat),
        table.game.host(),
    )
    .await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (status, code),
        "{}",
        reply.text()
    );
    assert_eq!(
        names(&server, &table.game.id).await,
        ["Ann", "Bo", "Cy", ""]
    );
}

#[tokio::test]
async fn the_host_lists_and_hides_the_game() {
    let server = roomy_server().await;
    let table = table(&server).await;
    let id = &table.game.id;
    let host = table.game.host();
    assert_eq!(shown_visibility(&server, id).await, "public");

    for (visibility, shown) in [
        ("invite", false),
        ("invite", false),
        ("public", true),
        ("public", true),
    ] {
        let body = json!({ "visibility": visibility }).to_string();
        let reply = patch(&server, id, host, &body).await;
        assert_eq!(reply.status, 200, "{}", reply.text());
        assert_eq!(reply.json(), json!({ "visibility": visibility }));
        assert_eq!(reply.header("cache-control"), Some("no-store"));
        let ids = listed(&lobbies(server.addr, &[]).await);
        assert_eq!(ids == [id.clone()], shown, "{visibility}");
        // A lobby page that reloads reads the same word from the lookup.
        assert_eq!(shown_visibility(&server, id).await, visibility);
    }
    // Hidden, the game is reached by its code and no longer by its id.
    patch(&server, id, host, r#"{"visibility":"invite"}"#).await;
    assert_eq!(
        join(server.addr, id, &json!({ "name": "Di" }), &[])
            .await
            .status,
        404
    );
    assert_eq!(
        lobby::join_code(server.addr, &table.game.code, Some("Di"))
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn only_the_host_switches_the_visibility() {
    let server = server().await;
    let table = table(&server).await;
    for token in [&table.bo, &table.cy] {
        let reply = patch(&server, &table.game.id, token, r#"{"visibility":"invite"}"#).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (403, "forbidden")
        );
    }
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        std::slice::from_ref(&table.game.id)
    );
}

#[rstest]
#[case::unknown_word(r#"{"visibility":"private"}"#, 400, "invalid_request")]
#[case::wrong_case(r#"{"visibility":"Public"}"#, 400, "invalid_request")]
#[case::null(r#"{"visibility":null}"#, 400, "invalid_request")]
#[case::missing(r"{}", 400, "invalid_request")]
#[case::unknown_field(r#"{"visibility":"public","held":1}"#, 400, "invalid_request")]
#[case::not_json("public", 400, "invalid_request")]
#[case::empty("", 400, "invalid_request")]
#[tokio::test]
async fn a_visibility_body_is_exact(#[case] body: &str, #[case] status: u16, #[case] code: &str) {
    let server = server().await;
    let table = table(&server).await;
    let reply = patch(&server, &table.game.id, table.game.host(), body).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (status, code),
        "{}",
        reply.text()
    );
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        std::slice::from_ref(&table.game.id)
    );
}

#[tokio::test]
async fn a_visibility_body_must_be_json() {
    let server = server().await;
    let table = table(&server).await;
    let reply = net::request(
        server.addr,
        "PATCH",
        &game_path(&table.game.id),
        &[
            ("Content-Type", "text/plain"),
            ("Authorization", &bearer(table.game.host())),
        ],
        br#"{"visibility":"invite"}"#,
    )
    .await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (415, "unsupported_media_type")
    );
}

#[tokio::test]
async fn after_the_start_every_change_answers_409_started() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let join_reply = join(server.addr, &game.id, &json!({ "name": "Bo" }), &[]).await;
    let bo = join_reply.json()["token"].as_str().unwrap().to_owned();
    lobby::start(&server.registry, &game.id, 2).await;

    let started = |reply: &Reply| (reply.status, reply.error_code());
    let want = (409, "started".to_owned());
    assert_eq!(
        started(&post_as(server.addr, &leave_path(&game.id), &bo).await),
        want
    );
    assert_eq!(
        started(&post_as(server.addr, &release_path(&game.id, "1"), game.host()).await),
        want
    );
    assert_eq!(
        started(&patch(&server, &game.id, game.host(), r#"{"visibility":"invite"}"#).await),
        want
    );
    assert_eq!(
        started(&join(server.addr, &game.id, &json!({ "name": "Cy" }), &[]).await),
        want
    );
    // Nothing moved: the token still opens the seat.
    assert_eq!(names(&server, &game.id).await, ["Ann", "Bo"]);
    let mut socket = net::connect(server.addr, &game.id, &bo).await;
    assert_eq!(socket.welcome(&game.id).await.index(), 1);
}

/// Every request that changes something is refused when another site makes it, however good the
/// credential it carries.
#[tokio::test]
async fn a_foreign_origin_is_refused_on_every_call_that_changes_something() {
    let server = roomy_server().await;
    let table = table(&server).await;
    let id = table.game.id.as_str();
    let evil = ("Origin", "https://evil.example");
    let host = bearer(table.game.host());
    let bo = bearer(&table.bo);
    let guest = json!({ "name": "Zed" }).to_string();
    let with_code = json!({ "code": table.game.code, "name": "Zed" }).to_string();
    let calls: Vec<(&str, String, Option<&str>, String)> = vec![
        ("POST", "/api/games".to_owned(), Some(&*guest), host.clone()),
        (
            "POST",
            format!("/api/games/{id}/join"),
            Some(&*guest),
            host.clone(),
        ),
        (
            "POST",
            "/api/join".to_owned(),
            Some(&*with_code),
            host.clone(),
        ),
        ("POST", leave_path(id), None, bo.clone()),
        ("POST", release_path(id, "1"), None, host.clone()),
        (
            "PATCH",
            game_path(id),
            Some(r#"{"visibility":"invite"}"#),
            host.clone(),
        ),
    ];
    for (method, path, body, authorization) in &calls {
        let reply = call(
            server.addr,
            method,
            path,
            *body,
            &[evil, ("Authorization", authorization)],
        )
        .await;
        assert_eq!(reply.status, 403, "{method} {path}: {}", reply.text());
        assert_eq!(reply.error_code(), "forbidden_origin", "{method} {path}");
    }
    // Nothing happened.
    assert_eq!(names(&server, id).await, ["Ann", "Bo", "Cy", ""]);
    assert_eq!(listed(&lobbies(server.addr, &[]).await), [id.to_owned()]);
    assert_eq!(server.registry.len(), 1);

    // The page the server itself serves may do all of it.
    let own = format!("http://{}", server.addr);
    let reply = call(
        server.addr,
        "POST",
        &leave_path(id),
        None,
        &[("Origin", &own), ("Authorization", &bo)],
    )
    .await;
    assert_eq!(reply.status, 204, "{}", reply.text());
    let reply = call(
        server.addr,
        "PATCH",
        &game_path(id),
        Some(r#"{"visibility":"invite"}"#),
        &[("Origin", &own), ("Authorization", &host)],
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
}

#[tokio::test]
async fn a_listed_origin_may_use_patch_the_bearer_header_and_read_the_tags() {
    let mut config = bot_config();
    config.allowed_origins = vec!["https://play.example".to_owned()];
    let server = TestServer::start(config).await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let page = ("Origin", "https://play.example");

    // The preflight a browser makes before the PATCH and the bearer token.
    let preflight = call(
        server.addr,
        "OPTIONS",
        &game_path(&game.id),
        None,
        &[
            page,
            ("Access-Control-Request-Method", "PATCH"),
            (
                "Access-Control-Request-Headers",
                "authorization,content-type",
            ),
        ],
    )
    .await;
    assert!(
        preflight.status == 200 || preflight.status == 204,
        "{}",
        preflight.status
    );
    let methods = preflight
        .header("access-control-allow-methods")
        .unwrap()
        .to_ascii_uppercase();
    assert!(methods.contains("PATCH"), "{methods}");
    let allowed = preflight
        .header("access-control-allow-headers")
        .unwrap()
        .to_ascii_lowercase();
    assert!(
        allowed.contains("authorization") && allowed.contains("content-type"),
        "{allowed}"
    );
    assert_eq!(
        preflight.header("access-control-allow-origin"),
        Some("https://play.example")
    );
    assert_eq!(
        preflight.header("access-control-allow-credentials"),
        Some("true")
    );

    // A poll with a tag needs its own header allowed, and the page must be allowed to read the tag.
    let poll = call(
        server.addr,
        "OPTIONS",
        "/api/lobbies",
        None,
        &[
            page,
            ("Access-Control-Request-Method", "GET"),
            ("Access-Control-Request-Headers", "if-none-match"),
        ],
    )
    .await;
    let allowed = poll
        .header("access-control-allow-headers")
        .unwrap()
        .to_ascii_lowercase();
    assert!(allowed.contains("if-none-match"), "{allowed}");
    let list = lobbies(server.addr, &[page]).await;
    let exposed = list
        .header("access-control-expose-headers")
        .unwrap()
        .to_ascii_lowercase();
    assert!(
        exposed.contains("etag") && exposed.contains("retry-after"),
        "{exposed}"
    );
    assert_eq!(
        list.header("access-control-allow-origin"),
        Some("https://play.example")
    );

    // And the call itself works from that origin.
    let hide = call(
        server.addr,
        "PATCH",
        &game_path(&game.id),
        Some(r#"{"visibility":"invite"}"#),
        &[page, ("Authorization", &bearer(game.host()))],
    )
    .await;
    assert_eq!(hide.status, 200, "{}", hide.text());
    let value: Value = hide.json();
    assert_eq!(value["visibility"], "invite");
}
