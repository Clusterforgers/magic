//! Goal: the host closes a game that is still in its lobby with `DELETE /api/games/{id}`: the answer is an empty 204, the game leaves the listing, the lookup, its code and every token at once (an invite game too), every socket of the game is closed with 4002, a player who is not the host is refused and nothing changes, a game that runs answers 409 started, a stranger and an unknown game get the one 404, and the actor settles a close racing the last Ready, so a game is either closed and never started or started and never closed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;

use std::time::{Duration, Instant};

use common::seat;
use mtg_core::ids::PlayerId;
use mtg_protocol::ClientMessage;
use mtg_server::registry::{
    ClaimError, Created, Entrance, GameOptions, HOST, Removal, SeatClaim, SeatRequest, TableError,
};
use mtg_server::{GameId, Registry, ServerConfig, Visibility, demo};
use net::lobby::{self, Made, bearer, call, join_code, listed, lobbies};
use net::{Reply, TestServer, WsClient, bot_config};
use rstest::rstest;
use serde_json::json;

/// A server that does not count list requests or wrong codes, for tests that ask often.
async fn server() -> TestServer {
    let mut config = bot_config();
    config.auth.limits.lobby_requests_per_ip = 100_000;
    config.auth.limits.code_misses_per_ip = 100_000;
    TestServer::start(config).await
}

fn game_path(game: &str) -> String {
    format!("/api/games/{game}")
}

/// `DELETE /api/games/{id}` presenting `token`.
async fn close_as(server: &TestServer, game: &str, token: &str) -> Reply {
    call(
        server.addr,
        "DELETE",
        &game_path(game),
        None,
        &[("Authorization", &bearer(token))],
    )
    .await
}

/// `GET /api/games/{id}`.
async fn look_up(server: &TestServer, game: &str) -> Reply {
    call(server.addr, "GET", &game_path(game), None, &[]).await
}

/// A game of four seats whose host holds seat 0 and which two guests, Bo and Cy, joined.
struct Table {
    game: Made,
    bo: String,
    cy: String,
}

async fn table(server: &TestServer, visibility: &str) -> Table {
    let game = lobby::make(server.addr, "Ann", 4, 1, visibility).await;
    let mut tokens = Vec::new();
    // The code reaches games of either visibility.
    for name in ["Bo", "Cy"] {
        let reply = join_code(server.addr, &game.code, Some(name)).await;
        assert_eq!(reply.status, 200, "{}", reply.text());
        tokens.push(reply.json()["token"].as_str().unwrap().to_owned());
    }
    Table {
        game,
        bo: tokens[0].clone(),
        cy: tokens[1].clone(),
    }
}

/// Connects a seat and reads its `Welcome`.
async fn seated(server: &TestServer, game: &str, token: &str, seat: usize) -> WsClient {
    let mut socket = net::connect(server.addr, game, token).await;
    assert_eq!(socket.welcome(game).await, PlayerId::from_index(seat));
    socket
}

fn not_found(reply: &Reply) -> bool {
    reply.status == 404 && reply.error_code() == "not_found"
}

#[rstest]
#[case::public("public")]
#[case::invite("invite")]
#[tokio::test]
async fn the_host_closes_a_lobby_and_it_is_gone_at_once(#[case] visibility: &str) {
    let server = server().await;
    let table = table(&server, visibility).await;
    let id = &table.game.id;
    assert_eq!(look_up(&server, id).await.status, 200);
    // Only a public game is listed.
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        if visibility == "public" {
            vec![id.clone()]
        } else {
            vec![]
        }
    );

    let closed = close_as(&server, id, table.game.host()).await;
    assert!(
        closed.status == 204 && closed.body.is_empty(),
        "{} {}",
        closed.status,
        closed.text()
    );
    assert_eq!(closed.header("cache-control"), Some("no-store"));

    // The very next look finds nothing: no listing, no lookup, no entry in the registry.
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        Vec::<String>::new()
    );
    assert!(not_found(&look_up(&server, id).await));
    assert!(server.registry.is_empty());
    // A second close finds no game to close.
    let again = close_as(&server, id, table.game.host()).await;
    assert!(not_found(&again), "{}", again.text());
}

#[rstest]
#[case::public("public")]
#[case::invite("invite")]
#[tokio::test]
async fn nothing_reaches_a_closed_game_any_more(#[case] visibility: &str) {
    let server = server().await;
    let table = table(&server, visibility).await;
    let id = &table.game.id;
    assert_eq!(close_as(&server, id, table.game.host()).await.status, 204);

    // Its id and its code lead nowhere.
    let by_id = call(
        server.addr,
        "POST",
        &format!("/api/games/{id}/join"),
        Some(&json!({ "name": "Di" }).to_string()),
        &[],
    )
    .await;
    assert!(not_found(&by_id), "{}", by_id.text());
    let by_code = join_code(server.addr, &table.game.code, Some("Di")).await;
    assert!(not_found(&by_code), "{}", by_code.text());

    // Neither do the tokens of its seats, over REST or over a socket.
    for token in [table.game.host(), &table.bo, &table.cy] {
        for (method, path, body) in [
            ("POST", format!("/api/games/{id}/leave"), None),
            ("POST", format!("/api/games/{id}/seats/1/release"), None),
            ("PATCH", game_path(id), Some(r#"{"visibility":"public"}"#)),
            ("DELETE", game_path(id), None),
        ] {
            let reply = call(
                server.addr,
                method,
                &path,
                body,
                &[("Authorization", &bearer(token))],
            )
            .await;
            assert!(not_found(&reply), "{method} {path}: {}", reply.text());
        }
        let (status, _) = net::refused(server.addr, id, token, None).await;
        assert_eq!(status, 404);
    }
}

#[rstest]
#[case::public("public")]
#[case::invite("invite")]
#[tokio::test]
async fn every_socket_of_the_game_is_closed_with_4002(#[case] visibility: &str) {
    let server = server().await;
    let table = table(&server, visibility).await;
    let id = &table.game.id;
    let mut host = seated(&server, id, table.game.host(), 0).await;
    let mut bo = seated(&server, id, &table.bo, 1).await;
    let mut cy = seated(&server, id, &table.cy, 2).await;

    let closed = close_as(&server, id, table.game.host()).await;
    assert_eq!(closed.status, 204, "{}", closed.text());

    for (who, socket) in [("host", &mut host), ("Bo", &mut bo), ("Cy", &mut cy)] {
        assert_eq!(socket.closed().await, Some(4002), "{who}");
    }
}

#[tokio::test]
async fn a_player_who_is_not_the_host_cannot_close_the_game() {
    let server = server().await;
    // Seat 1 was made with the game, Bo took seat 2 through the listing.
    let game = lobby::make(server.addr, "Ann", 4, 2, "public").await;
    let joined = call(
        server.addr,
        "POST",
        &format!("/api/games/{}/join", game.id),
        Some(&json!({ "name": "Bo" }).to_string()),
        &[],
    )
    .await;
    let bo = joined.json()["token"].as_str().unwrap().to_owned();
    let mut host = seated(&server, &game.id, game.host(), 0).await;
    let mut other = seated(&server, &game.id, &game.tokens[1], 1).await;

    for (who, token) in [("the other player", game.tokens[1].as_str()), ("Bo", &bo)] {
        let reply = close_as(&server, &game.id, token).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (403, "forbidden"),
            "{who}: {}",
            reply.text()
        );
    }

    // Nothing changed: the game is there, listed, with its seats, and the sockets stay open.
    assert_eq!(
        listed(&lobbies(server.addr, &[]).await),
        std::slice::from_ref(&game.id)
    );
    let view = look_up(&server, &game.id).await.json();
    assert_eq!(view["seats"][2]["name"], "Bo");
    other
        .send(&ClientMessage::Chat {
            text: "still here".into(),
        })
        .await;
    loop {
        if let mtg_protocol::ServerMessage::Chat { text, .. } = host.message().await {
            assert_eq!(text, "still here");
            break;
        }
    }
    assert_eq!(server.registry.len(), 1);
}

#[tokio::test]
async fn a_game_that_runs_cannot_be_closed() {
    let server = server().await;
    let game = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let joined = call(
        server.addr,
        "POST",
        &format!("/api/games/{}/join", game.id),
        Some(&json!({ "name": "Bo" }).to_string()),
        &[],
    )
    .await;
    let bo = joined.json()["token"].as_str().unwrap().to_owned();
    lobby::start(&server.registry, &game.id, 2).await;

    let host = close_as(&server, &game.id, game.host()).await;
    assert_eq!(
        (host.status, host.error_code().as_str()),
        (409, "started"),
        "{}",
        host.text()
    );
    // Who asks is judged before what state the game is in: a joiner is not the host either way.
    let refused = close_as(&server, &game.id, &bo).await;
    assert_eq!(
        (refused.status, refused.error_code().as_str()),
        (403, "forbidden")
    );

    // The game goes on: it is there, running, and its seats still connect.
    let view = look_up(&server, &game.id).await.json();
    assert_eq!(view["started"], true);
    let mut socket = seated(&server, &game.id, game.host(), 0).await;
    socket.close().await;
    assert_eq!(server.registry.len(), 1);
}

/// What a request with no standing at the table is told, whatever it asks for.
#[tokio::test]
async fn a_stranger_and_an_unknown_game_get_the_one_404() {
    let server = server().await;
    let table = table(&server, "public").await;
    let id = table.game.id.as_str();
    let other = lobby::make(server.addr, "Zed", 2, 2, "invite").await;
    let unknown = "6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55";
    let wrong_token = "0".repeat(64);

    // The answer every other host call gives to the same stranger is the one to match.
    let model = call(
        server.addr,
        "PATCH",
        &game_path(unknown),
        Some(r#"{"visibility":"invite"}"#),
        &[("Authorization", &bearer(&wrong_token))],
    )
    .await;
    assert_eq!(
        model.json(),
        json!({"error": "not_found", "message": "no such resource"})
    );

    let mut answers: Vec<(String, Reply)> = Vec::new();
    for (what, token) in [
        ("a wrong token", wrong_token.as_str()),
        ("a token of another game", other.host()),
        ("a malformed token", "not a token"),
    ] {
        for game in [id, unknown, "not-a-game"] {
            answers.push((
                format!("{what} for {game}"),
                close_as(&server, game, token).await,
            ));
        }
    }
    // A real host's token does not open a game that is not there.
    for game in [unknown, "not-a-game"] {
        answers.push((
            format!("the host's token for {game}"),
            close_as(&server, game, table.game.host()).await,
        ));
    }
    for (what, reply) in &answers {
        assert_eq!(reply.status, 404, "{what}: {}", reply.text());
        assert_eq!(reply.body, model.body, "{what}");
    }
    // Nothing was closed.
    assert_eq!(listed(&lobbies(server.addr, &[]).await), [id.to_owned()]);
    assert_eq!(server.registry.len(), 2);
}

#[tokio::test]
async fn without_a_credential_a_server_with_no_accounts_says_so() {
    let server = server().await;
    let table = table(&server, "public").await;
    let id = table.game.id.as_str();
    for headers in [
        vec![],
        vec![("Cookie", "mtg_session=0123456789abcdef")],
        vec![("Authorization", "Basic dXNlcjpwYXNz")],
        vec![("Authorization", "Bearer")],
    ] {
        let reply = call(server.addr, "DELETE", &game_path(id), None, &headers).await;
        assert_eq!(reply.status, 503, "{headers:?}: {}", reply.text());
        assert_eq!(reply.error_code(), "accounts_unavailable");
    }
    assert_eq!(server.registry.len(), 1);
}

#[tokio::test]
async fn a_close_that_comes_after_the_last_ready_is_refused_and_one_before_it_wins() {
    let server = server().await;

    // The last Ready comes first: the game runs and the close is too late.
    let late = lobby::make(server.addr, "Ann", 2, 2, "public").await;
    lobby::start(&server.registry, &late.id, 2).await;
    let reply = close_as(&server, &late.id, late.host()).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (409, "started")
    );

    // The close comes first: the Ready that follows it starts nothing.
    let early = lobby::make(server.addr, "Cy", 2, 2, "public").await;
    let handle = server.registry.handle(early.id.parse().unwrap()).unwrap();
    handle.send(seat(0), ClientMessage::Ready).await.unwrap();
    assert_eq!(close_as(&server, &early.id, early.host()).await.status, 204);
    handle.send(seat(1), ClientMessage::Ready).await.unwrap();
    let status = handle.status().await.unwrap();
    assert!(!status.started, "a closed game must not start");
    assert!(not_found(&look_up(&server, &early.id).await));
}

// --- the registry ---------------------------------------------------------------------------

fn requests(seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

/// A lobby of `seats` seats of which only the host's is held.
async fn lobby_of(registry: &Registry, seats: usize, visibility: Visibility) -> Created {
    let options = GameOptions {
        visibility,
        held: Some(1),
    };
    registry
        .create_with(&requests(seats), options, None)
        .await
        .unwrap()
}

async fn guest(registry: &Registry, game: GameId, name: &str) -> Result<(), ClaimError> {
    let claim = SeatClaim {
        name: name.to_owned(),
        user: None,
    };
    registry
        .claim(game, Entrance::Code, claim)
        .await
        .map(|_| ())
}

#[tokio::test]
async fn the_registry_closes_a_lobby_for_the_host_alone() {
    let registry = Registry::new(&ServerConfig::default());
    let created = lobby_of(&registry, 3, Visibility::Public).await;
    let game = created.id;
    guest(&registry, game, "Bo").await.unwrap();

    // A player who is not the host and a seat the table does not have are refused, and the game
    // is untouched; an unknown game is unknown whoever asks.
    for by in [seat(1), seat(2), seat(7)] {
        assert_eq!(registry.close(game, by).await, Err(TableError::NotAllowed));
    }
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.lobbies().await.len(), 1);
    assert_eq!(
        registry.close(GameId::random(), HOST).await,
        Err(TableError::NotFound)
    );

    assert_eq!(registry.close(game, HOST).await, Ok(()));
    assert!(registry.is_empty());
    assert_eq!(registry.lobbies().await, []);
    assert!(registry.describe(game).await.is_none());
    assert!(registry.handle(game).is_none());
    assert!(registry.visibility(game).is_none());
    assert_eq!(registry.close(game, HOST).await, Err(TableError::NotFound));
}

#[tokio::test]
async fn the_registry_does_not_close_a_game_that_runs() {
    let registry = Registry::new(&ServerConfig::default());
    let created = registry.create(&requests(2)).await.unwrap();
    let game = created.id;
    let handle = registry.handle(game).unwrap();
    for n in 0..2 {
        handle.send(seat(n), ClientMessage::Ready).await.unwrap();
    }
    assert!(handle.status().await.unwrap().started);

    assert_eq!(registry.close(game, HOST).await, Err(TableError::Started));
    assert_eq!(registry.len(), 1);
    assert!(registry.describe(game).await.unwrap().started);
}

#[tokio::test]
async fn the_sockets_are_told_why_the_game_left_the_registry() {
    let registry = Registry::new(&ServerConfig::default());
    let closed = registry.create(&requests(2)).await.unwrap();
    let dropped = registry.create(&requests(2)).await.unwrap();
    let removed = |created: &Created| {
        let (_, token) = &created.seats[1];
        registry
            .authenticate(created.id, token.expose())
            .unwrap()
            .removed
    };
    let (mut by_host, mut by_janitor) = (removed(&closed), removed(&dropped));
    assert_eq!(*by_host.borrow(), Removal::Dropped, "nothing has left yet");

    registry.close(closed.id, HOST).await.unwrap();
    // Either way the wait ends (the channel is closed with the entry); the value says why.
    let _ = by_host.changed().await;
    assert_eq!(*by_host.borrow(), Removal::ClosedByHost);

    // A game the janitor drops leaves the plain reason, and the other game is not touched.
    assert_eq!(
        registry
            .sweep(Instant::now() + Duration::from_hours(2))
            .await,
        1
    );
    assert!(by_janitor.changed().await.is_err());
    assert_eq!(*by_janitor.borrow(), Removal::Dropped);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_close_racing_the_last_ready_leaves_one_consistent_outcome() {
    let registry = Registry::new(&ServerConfig::default());
    let (mut closed, mut began) = (0, 0);
    for round in 0..60 {
        let created = registry.create(&requests(2)).await.unwrap();
        let game = created.id;
        let handle = registry.handle(game).unwrap();
        handle.send(seat(0), ClientMessage::Ready).await.unwrap();

        let ready = {
            let handle = handle.clone();
            tokio::spawn(async move {
                // Each round lets the Ready go a little later, so it lands on both sides of the close.
                for _ in 0..round % 12 {
                    tokio::task::yield_now().await;
                }
                handle.send(seat(1), ClientMessage::Ready).await
            })
        };
        let closing = {
            let registry = registry.clone();
            tokio::spawn(async move { registry.close(game, HOST).await })
        };
        let (answer, sent) = (closing.await.unwrap(), ready.await.unwrap());
        sent.unwrap();

        let started = handle.status().await.unwrap().started;
        match answer {
            Ok(()) => {
                assert!(
                    !started,
                    "round {round}: the game began after it was closed"
                );
                assert!(registry.describe(game).await.is_none(), "round {round}");
                closed += 1;
            }
            Err(TableError::Started) => {
                assert!(
                    started,
                    "round {round}: refused as started, yet it never began"
                );
                assert!(registry.describe(game).await.is_some(), "round {round}");
                began += 1;
            }
            Err(other) => panic!("round {round}: {other}"),
        }
    }
    assert_eq!(closed + began, 60);
}
