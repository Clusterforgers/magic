//! Goal: games and accounts meet correctly on Postgres: a signed-in creator owns seat 0, the first signed-in visitor of a seat link claims the seat and nobody can take a claimed seat or hold two in a game, guests and stale cookies play exactly as before, `/api/me/games` lists the user's lobbies, running and finished games newest first with the other seats, a bound seat is rejoined with the session alone (and refused to everyone else), a whole game is played through real sockets by a signed-in user who rejoins after a disconnect, and all of it survives a restart.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::bot::play_table;
use mtg_core::ids::PlayerId;
use mtg_engine::GameSetup;
use mtg_engine::format::Normal;
use mtg_engine::state::GameResult;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{Binding, GameId, Registry, ServerConfig, UserId, demo, migrate};
use mtg_testkit::{GameOutcome, play_game};
use net::bot::Stop;
use net::{
    Browser, PASSWORD, TestServer, WsBot, WsClient, bot_config, open, refused_at, register,
    request, two_seats,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

async fn start_with(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> TestServer {
    migrate(pool).await.unwrap();
    let mut config = bot_config();
    config.auth.limits.login_per_ip = 10_000;
    config.auth.limits.login_per_username = 10_000;
    config.auth.limits.register_per_ip = 10_000;
    change(&mut config);
    TestServer::start_on(pool.clone(), config).await
}

async fn start(pool: &PgPool) -> TestServer {
    start_with(pool, |_| {}).await
}

/// The account each seat of `game` belongs to, in seat order.
async fn owners(pool: &PgPool, game: &str) -> Vec<Option<String>> {
    sqlx::query_scalar("SELECT user_id::text FROM seats WHERE game_id = $1::uuid ORDER BY seat")
        .bind(game)
        .fetch_all(pool)
        .await
        .unwrap()
}

/// Opens a seat's link (token in the path), with the browser's cookie when there is one, and joins.
async fn through_the_link(
    addr: std::net::SocketAddr,
    game: &str,
    token: &str,
    browser: Option<&Browser>,
) -> (WsClient, usize) {
    let headers: Vec<(&str, &str)> = browser
        .map(|b| ("Cookie", b.cookie.as_str()))
        .into_iter()
        .collect();
    let mut client = open(addr, &format!("/ws/{game}/{token}"), &headers).await;
    let seat = client.welcome(game).await.index();
    (client, seat)
}

/// Opens the signed-in socket (no token) and joins.
async fn through_the_session(
    addr: std::net::SocketAddr,
    game: &str,
    browser: &Browser,
) -> (WsClient, usize) {
    let mut client = open(addr, &format!("/ws/{game}"), &[("Cookie", &browser.cookie)]).await;
    let seat = client.welcome(game).await.index();
    (client, seat)
}

/// One request the session socket must refuse: what it is, the path, the headers, and the status
/// and error code expected.
type Refusal<'a> = (&'a str, &'a str, Vec<(&'a str, &'a str)>, u16, &'a str);

fn host_game() -> Value {
    json!({ "format": "normal", "seats": [
        { "name": "Host", "deck": "mono-green" },
        { "name": "Friend", "deck": "mono-red" },
    ]})
}

fn handle_of(server: &TestServer, game: &str) -> mtg_server::GameHandle {
    server.registry.handle(game.parse().unwrap()).unwrap()
}

/// Presses Ready for every seat, so the game starts.
async fn start_game(server: &TestServer, game: &str) {
    let handle = handle_of(server, game);
    for seat in 0..2 {
        let ready = ClientMessage::Ready;
        handle
            .send(PlayerId::from_index(seat), ready)
            .await
            .unwrap();
    }
    assert!(handle.status().await.unwrap().started);
}

/// Plays the game to its end with bots on the handle.
async fn finish_game(server: &TestServer, game: &str) -> GameResult {
    let handle = handle_of(server, game);
    play_table(&handle, 2, 7).await[0].result
}

async fn pause() {
    // Games created in a row must differ in their creation time to be ordered by it.
    tokio::time::sleep(Duration::from_millis(15)).await;
}

#[sqlx::test(migrations = false)]
async fn a_signed_in_creator_owns_seat_zero_and_nobody_else_is_bound(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let gone = register(addr, "Gone", PASSWORD).await;
    assert_eq!(gone.post(addr, "/api/auth/logout", "").await.status, 204);

    let (mine, _) = ann.create_game(addr, &host_game()).await;
    let (guests, _) = net::create_game(addr, host_game()).await;
    // A cookie the server does not honour makes the creator a guest, not an error.
    let reply = gone
        .post(addr, "/api/games", &host_game().to_string())
        .await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let stale = reply.json()["game"].as_str().unwrap().to_owned();

    assert_eq!(owners(&pool, &mine).await, [Some(ann.id.clone()), None]);
    assert_eq!(owners(&pool, &guests).await, [None, None]);
    assert_eq!(owners(&pool, &stale).await, [None, None]);
    // The response is the one guests always got.
    let keys: Vec<String> = reply.json().as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["code", "game", "seats"]);
}

#[sqlx::test(migrations = false)]
async fn the_first_signed_in_visitor_of_a_seat_link_claims_the_seat_and_nobody_can_take_it(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let addr = server.addr;
    let (ann, bob, cy) = (
        register(addr, "Ann", PASSWORD).await,
        register(addr, "Bob", PASSWORD).await,
        register(addr, "Cyd", PASSWORD).await,
    );
    let (game, tokens) = ann.create_game(addr, &host_game()).await;
    assert_eq!(owners(&pool, &game).await, [Some(ann.id.clone()), None]);

    // Bob opens the second seat's link: the seat is his.
    let (mut bobs_socket, seat) = through_the_link(addr, &game, &tokens[1], Some(&bob)).await;
    assert_eq!(seat, 1);
    assert_eq!(
        owners(&pool, &game).await,
        [Some(ann.id.clone()), Some(bob.id.clone())]
    );

    // Cy follows the same link later. The token still opens the seat (and replaces Bob's socket)
    // but the seat stays Bob's and Cy gets no game.
    let (_cys_socket, seat) = through_the_link(addr, &game, &tokens[1], Some(&cy)).await;
    assert_eq!(seat, 1);
    assert_eq!(bobs_socket.closed().await, Some(4000));
    assert_eq!(
        owners(&pool, &game).await,
        [Some(ann.id.clone()), Some(bob.id.clone())]
    );
    assert_eq!(cy.games(addr).await.len(), 0);

    // Nor does a stranger with Ann's link take Ann's seat, and a guest still gets in.
    let (_, seat) = through_the_link(addr, &game, &tokens[0], Some(&bob)).await;
    assert_eq!(seat, 0);
    let (_, seat) = through_the_link(addr, &game, &tokens[0], None).await;
    assert_eq!(seat, 0);
    assert_eq!(
        owners(&pool, &game).await,
        [Some(ann.id.clone()), Some(bob.id.clone())]
    );

    // An account holds one seat per game: Ann opening the second link changes nothing.
    let (_, seat) = through_the_link(addr, &game, &tokens[1], Some(&ann)).await;
    assert_eq!(seat, 1);
    assert_eq!(
        owners(&pool, &game).await,
        [Some(ann.id.clone()), Some(bob.id.clone())]
    );

    // Everyone who is bound can come back with the session alone.
    for (browser, expected) in [(&ann, 0), (&bob, 1)] {
        let (_, seat) = through_the_session(addr, &game, browser).await;
        assert_eq!(seat, expected, "{}", browser.username);
    }
}

#[sqlx::test(migrations = false)]
async fn racing_accounts_get_one_winner_for_a_seat_and_one_seat_each(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let (bob, cy) = (
        register(addr, "Bob", PASSWORD).await,
        register(addr, "Cyd", PASSWORD).await,
    );
    for round in 0..8 {
        // Two accounts open the same unclaimed link at once.
        let (game, tokens) = net::create_game(addr, host_game()).await;
        let (a, b) = tokio::join!(
            through_the_link(addr, &game, &tokens[0], Some(&bob)),
            through_the_link(addr, &game, &tokens[0], Some(&cy)),
        );
        assert_eq!(
            (a.1, b.1),
            (0, 0),
            "round {round}: both reached the seat by token"
        );
        let bound = owners(&pool, &game).await;
        assert_eq!(bound.len(), 2);
        assert!(
            bound[0] == Some(bob.id.clone()) || bound[0] == Some(cy.id.clone()),
            "round {round}: {bound:?}"
        );
        assert_eq!(bound[1], None);
        let (bobs, cys) = (bob.games(addr).await, cy.games(addr).await);
        let listed = |games: &[Value]| games.iter().filter(|g| g["game"] == game.as_str()).count();
        assert_eq!(
            listed(&bobs) + listed(&cys),
            1,
            "round {round}: exactly one list has it"
        );

        // One account opens both links at once: it ends up with one seat.
        let (game, tokens) = net::create_game(addr, host_game()).await;
        let _ = tokio::join!(
            through_the_link(addr, &game, &tokens[0], Some(&bob)),
            through_the_link(addr, &game, &tokens[1], Some(&bob)),
        );
        let bound = owners(&pool, &game).await;
        assert_eq!(
            bound.iter().filter(|owner| owner.is_some()).count(),
            1,
            "round {round}: {bound:?}"
        );
    }
}

#[sqlx::test(migrations = false)]
async fn a_bound_seat_is_rejoined_with_the_session_alone_and_to_nobody_else(pool: PgPool) {
    let own_origin = |server: &TestServer| format!("http://{}", server.addr);
    let server = start(&pool).await;
    let addr = server.addr;
    let (ann, bob) = (
        register(addr, "Ann", PASSWORD).await,
        register(addr, "Bob", PASSWORD).await,
    );
    let (game, tokens) = ann.create_game(addr, &host_game()).await;
    let nowhere = Uuid::new_v4();
    let fake_session = format!("mtg_session={}", "ab".repeat(32));

    let origin = own_origin(&server);
    let foreign = "https://evil.example";
    let (ann_cookie, bob_cookie) = (ann.cookie.as_str(), bob.cookie.as_str());
    let (in_game, no_game) = (format!("/ws/{game}"), format!("/ws/{nowhere}"));
    let cases: Vec<Refusal> = vec![
        ("no cookie", &in_game, vec![], 401, "unauthenticated"),
        (
            "an unknown session",
            &in_game,
            vec![("Cookie", fake_session.as_str())],
            401,
            "unauthenticated",
        ),
        (
            "a user without a seat here",
            &in_game,
            vec![("Cookie", bob_cookie)],
            404,
            "not_found",
        ),
        (
            "an unknown game",
            &no_game,
            vec![("Cookie", ann_cookie)],
            404,
            "not_found",
        ),
        (
            "a malformed id",
            "/ws/not-a-game",
            vec![("Cookie", ann_cookie)],
            404,
            "not_found",
        ),
        (
            "a malformed id and no cookie",
            "/ws/not-a-game",
            vec![],
            404,
            "not_found",
        ),
        (
            "another site",
            &in_game,
            vec![("Cookie", ann_cookie), ("Origin", foreign)],
            403,
            "forbidden_origin",
        ),
        (
            "a seat token in place of the cookie",
            &in_game,
            vec![("Cookie", tokens[0].as_str())],
            401,
            "unauthenticated",
        ),
    ];
    for (what, path, headers, status, code) in cases {
        let (got, body) = refused_at(addr, path, &headers).await;
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            (got, body["error"].as_str().unwrap()),
            (status, code),
            "{what}"
        );
    }

    // The same request from the server's own page, or from a client that sends no Origin, is let in.
    for headers in [
        vec![("Cookie", ann.cookie.as_str()), ("Origin", origin.as_str())],
        vec![("Cookie", ann.cookie.as_str())],
    ] {
        let mut client = open(addr, &format!("/ws/{game}"), &headers).await;
        assert_eq!(client.welcome(&game).await.index(), 0);
    }

    // Signing out ends the right to come back this way, and not the link's.
    assert_eq!(ann.post(addr, "/api/auth/logout", "").await.status, 204);
    let (status, _) = refused_at(addr, &format!("/ws/{game}"), &[("Cookie", &ann.cookie)]).await;
    assert_eq!(status, 401);
    let (_, seat) = through_the_link(addr, &game, &tokens[0], None).await;
    assert_eq!(seat, 0);
}

#[tokio::test]
async fn a_server_without_a_database_has_no_signed_in_sockets_or_lists() {
    let server = TestServer::start(bot_config()).await;
    let addr = server.addr;
    let (game, tokens) = net::create_game(addr, host_game()).await;
    let stray = "mtg_session=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    let (status, body) = refused_at(addr, &format!("/ws/{game}"), &[("Cookie", stray)]).await;
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (503, "accounts_unavailable")
    );
    let list = request(addr, "GET", "/api/me/games", &[("Cookie", stray)], b"").await;
    assert_eq!(
        (list.status, list.error_code().as_str()),
        (503, "accounts_unavailable")
    );
    // The seat link, with or without a cookie nobody can check, is the way in as ever.
    for browser_cookie in [None, Some(stray)] {
        let headers: Vec<(&str, &str)> =
            browser_cookie.map(|c| ("Cookie", c)).into_iter().collect();
        let mut client = open(addr, &format!("/ws/{game}/{}", tokens[1]), &headers).await;
        assert_eq!(client.welcome(&game).await.index(), 1);
    }
}

#[sqlx::test(migrations = false)]
async fn the_list_has_lobbies_running_and_finished_games_newest_first_with_the_other_seats(
    pool: PgPool,
) {
    let server = start_with(&pool, |config| config.fixed_seed = Some(5)).await;
    let addr = server.addr;
    let (ann, bob) = (
        register(addr, "Ann", PASSWORD).await,
        register(addr, "Bob", PASSWORD).await,
    );

    // g1: a lobby Ann made, with Bob in the second seat.
    let (g1, tokens) = ann.create_game(addr, &host_game()).await;
    let _ = through_the_link(addr, &g1, &tokens[1], Some(&bob)).await;
    pause().await;
    // g2: a game under way, with a guest in the second seat.
    let (g2, _) = ann.create_game(addr, &host_game()).await;
    start_game(&server, &g2).await;
    pause().await;
    // g3: a game that ended.
    let (g3, _) = ann.create_game(addr, &host_game()).await;
    let result = finish_game(&server, &g3).await;
    pause().await;
    // g4: Bob's game, in which Ann holds the second seat.
    let (g4, tokens) = bob.create_game(addr, &host_game()).await;
    let _ = through_the_link(addr, &g4, &tokens[1], Some(&ann)).await;
    pause().await;
    // g5: Bob's game, which Ann never touched. g6 is Ann's, but dropped.
    let (g5, _) = bob.create_game(addr, &host_game()).await;
    let (g6, _) = ann.create_game(addr, &host_game()).await;
    sqlx::query("UPDATE games SET status = 'expired' WHERE id = $1::uuid")
        .bind(&g6)
        .execute(&pool)
        .await
        .unwrap();

    let games = ann.games(addr).await;
    let ids: Vec<&str> = games.iter().map(|g| g["game"].as_str().unwrap()).collect();
    assert_eq!(ids, [g4.as_str(), g3.as_str(), g2.as_str(), g1.as_str()]);
    assert!(!ids.contains(&g5.as_str()) && !ids.contains(&g6.as_str()));

    let [listed4, listed3, listed2, listed1] = &games[..] else {
        panic!("four games expected");
    };
    // Where the user sits, what the game is and what became of it.
    assert_eq!(listed1["seat"], 0);
    assert_eq!(listed1["name"], "Host");
    assert_eq!(listed1["status"], "lobby");
    assert_eq!(listed1["result"], Value::Null);
    assert_eq!(
        listed1["others"],
        json!([{"seat": 1, "name": "Friend", "user": "Bob"}])
    );
    assert_eq!(listed2["status"], "running");
    assert_eq!(
        listed2["others"],
        json!([{"seat": 1, "name": "Friend", "user": null}])
    );
    assert_eq!(listed3["status"], "finished");
    assert_eq!(listed3["result"], serde_json::to_value(result).unwrap());
    assert_eq!(listed4["seat"], 1);
    assert_eq!(listed4["name"], "Friend");
    assert_eq!(listed4["status"], "lobby");
    assert_eq!(
        listed4["others"],
        json!([{"seat": 0, "name": "Host", "user": "Bob"}])
    );
    // Creation times come from the database in one fixed shape, newest first.
    let times: Vec<&str> = games
        .iter()
        .map(|g| g["created_at"].as_str().unwrap())
        .collect();
    for time in &times {
        assert_eq!(time.len(), 20, "{time}");
        let shape: String = time
            .chars()
            .map(|c| if c.is_ascii_digit() { '9' } else { c })
            .collect();
        assert_eq!(shape, "9999-99-99T99:99:99Z");
    }
    assert!(times.windows(2).all(|pair| pair[0] >= pair[1]), "{times:?}");

    // Bob's list is his own: his two games and Ann's lobby he sits in.
    let bobs = bob.games(addr).await;
    let bobs: Vec<&str> = bobs.iter().map(|g| g["game"].as_str().unwrap()).collect();
    assert_eq!(bobs, [g5.as_str(), g4.as_str(), g1.as_str()]);

    // Every item has exactly the documented keys.
    for game in &games {
        let mut keys: Vec<&str> = game
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "created_at",
                "game",
                "name",
                "others",
                "reason",
                "result",
                "seat",
                "status"
            ]
        );
    }
}

#[sqlx::test(migrations = false)]
async fn the_list_needs_a_session_and_shows_only_the_newest_hundred(pool: PgPool) {
    let server = start_with(&pool, |config| config.max_games = 150).await;
    let addr = server.addr;
    let anonymous = request(addr, "GET", "/api/me/games", &[], b"").await;
    assert_eq!(
        (anonymous.status, anonymous.error_code().as_str()),
        (401, "unauthenticated")
    );
    assert_eq!(anonymous.header("cache-control"), Some("no-store"));
    let stale = format!("mtg_session={}", "cd".repeat(32));
    let told = request(addr, "GET", "/api/me/games", &[("Cookie", &stale)], b"").await;
    assert_eq!(told.status, 401);
    assert!(told.headers_named("set-cookie")[0].contains("Max-Age=0"));
    // Only GET reads the list.
    let post = request(addr, "POST", "/api/me/games", &[], b"").await;
    assert_eq!(post.status, 405);

    let ann = register(addr, "Ann", PASSWORD).await;
    let empty = ann.games(addr).await;
    assert_eq!(empty.len(), 0);
    let mut made = Vec::new();
    for _ in 0..104 {
        made.push(ann.create_game(addr, &host_game()).await.0);
    }
    let games = ann.games(addr).await;
    assert_eq!(games.len(), 100);
    // The newest hundred, the newest first.
    let ids: Vec<&str> = games.iter().map(|g| g["game"].as_str().unwrap()).collect();
    let expected: Vec<&str> = made.iter().rev().take(100).map(String::as_str).collect();
    assert_eq!(ids.len(), expected.len());
    let listed: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
    let newest: std::collections::BTreeSet<&str> = expected.iter().copied().collect();
    assert_eq!(listed, newest, "the oldest four are the ones left out");
}

#[sqlx::test(migrations = false)]
async fn finished_games_stay_listed_after_the_server_forgets_them(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let (game, tokens) = ann.create_game(addr, &host_game()).await;
    let result = finish_game(&server, &game).await;

    // The janitor drops a finished game ten minutes after it noticed the end.
    server.registry.sweep(Instant::now()).await;
    let later = Instant::now() + Duration::from_mins(11);
    assert_eq!(server.registry.sweep(later).await, 1);
    assert!(
        server
            .registry
            .handle(game.parse::<GameId>().unwrap())
            .is_none()
    );

    let games = ann.games(addr).await;
    assert_eq!(games.len(), 1);
    assert_eq!(games[0]["status"], "finished");
    assert_eq!(games[0]["result"], serde_json::to_value(result).unwrap());
    // Nothing left to rejoin, by session or by link.
    let (status, _) = refused_at(addr, &format!("/ws/{game}"), &[("Cookie", &ann.cookie)]).await;
    assert_eq!(status, 404);
    let (status, _) = refused_at(addr, &format!("/ws/{game}/{}", tokens[0]), &[]).await;
    assert_eq!(status, 404);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comeback {
    /// The old socket dies and the server notices before the user returns.
    HangUp,
    /// The user opens a second socket while the first is still open.
    Replace,
}

/// The same game played straight on an engine with the same bots and seed.
fn straight(seed: u64) -> GameOutcome {
    let decks = ["mono-green", "mono-red"]
        .iter()
        .map(|id| demo::find(id).unwrap().cards())
        .collect();
    let setup = GameSetup::new(Arc::new(Normal), seed, decks);
    play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"))
}

#[sqlx::test(migrations = false)]
async fn a_signed_in_user_plays_a_whole_game_over_real_sockets_and_rejoins_with_the_session_after_a_disconnect(
    pool: PgPool,
) {
    let seed = 3;
    let alone = straight(seed);
    let server = start_with(&pool, |config| config.fixed_seed = Some(seed)).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;

    for (round, comeback) in [Comeback::HangUp, Comeback::Replace]
        .into_iter()
        .enumerate()
    {
        let (game, tokens) = ann.create_game(addr, &host_game()).await;
        let handle = handle_of(&server, &game);
        let seat0 = PlayerId::from_index(0);

        // Ann takes seat 0 through the session, a guest takes seat 1 with the link.
        let mut user = WsBot::connect_as_user(addr, &game, &ann.cookie, 0, seed).await;
        let mut guest = WsBot::connect(addr, &game, &tokens[1], 1, seed).await;
        user.pause_at = Some(7 + 5 * round);
        guest.ready().await;
        let guest = tokio::spawn(async move { guest.finish().await });
        user.ready().await;

        // She plays until she faces the n-th decision, then leaves with the answer unsent. The game
        // is under way and on her list meanwhile.
        assert_eq!(user.play().await, Stop::Paused);
        assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat0));
        let listed = ann.games(addr).await;
        assert_eq!(listed[0]["game"], game.as_str());
        assert_eq!(listed[0]["status"], "running");
        if comeback == Comeback::HangUp {
            user.client.hang_up();
            for _ in 0..200 {
                if !handle.status().await.unwrap().connected.contains(&seat0) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            assert!(!handle.status().await.unwrap().connected.contains(&seat0));
        }

        // She comes back with the session alone and is brought up to date.
        let mut previous = user.reconnect_as_user(addr, &game, &ann.cookie).await;
        if comeback == Comeback::Replace {
            assert_eq!(
                previous.closed().await,
                Some(4000),
                "the old socket is told"
            );
        }
        let first = [
            user.client.message().await,
            user.client.message().await,
            user.client.message().await,
        ];
        assert!(matches!(&first[0], ServerMessage::Welcome { player, .. } if *player == seat0));
        assert!(
            matches!(&first[1], ServerMessage::Snapshot { state, .. } if state.table.viewer == seat0)
        );
        assert!(
            matches!(&first[2], ServerMessage::Decision { decision, .. } if decision.player == seat0)
        );
        for message in first {
            assert_eq!(user.feed(message).await, None);
        }

        // The game ends exactly as the same game does without any of this.
        let report = user.finish().await;
        assert_eq!(report.snapshots, 1, "{comeback:?}");
        assert_eq!(report.result, alone.result);
        assert_eq!(report.events_seen, alone.events.len() as u64);
        assert_eq!(report.errors, []);
        let guest = guest.await.unwrap();
        assert_eq!(guest.result, alone.result);
        let status = handle.status().await.unwrap();
        assert_eq!(status.state_hash, Some(alone.final_hash));
        assert_eq!(status.result, Some(alone.result));

        // And it is on her list as finished, with the result.
        let listed = ann.games(addr).await;
        assert_eq!(listed[0]["game"], game.as_str());
        assert_eq!(listed[0]["status"], "finished");
        assert_eq!(
            listed[0]["result"],
            serde_json::to_value(alone.result).unwrap()
        );
    }
}

#[sqlx::test(migrations = false)]
async fn guests_and_stale_cookies_keep_playing_through_their_links(pool: PgPool) {
    let seed = 4;
    let alone = straight(seed);
    let server = start_with(&pool, |config| config.fixed_seed = Some(seed)).await;
    let addr = server.addr;
    let (game, tokens) = net::create_game(addr, two_seats()).await;
    // One guest sends no cookie, the other a cookie that opens nothing.
    let stale = format!("mtg_session={}", "ef".repeat(32));
    let mut clients = Vec::new();
    for (seat, cookie) in [(0, None), (1, Some(stale.as_str()))] {
        let headers: Vec<(&str, &str)> = cookie.map(|c| ("Cookie", c)).into_iter().collect();
        let mut client = open(addr, &format!("/ws/{game}/{}", tokens[seat]), &headers).await;
        assert_eq!(client.welcome(&game).await.index(), seat);
        clients.push(client);
    }
    drop(clients);
    let bots = [
        WsBot::connect(addr, &game, &tokens[0], 0, seed).await,
        WsBot::connect(addr, &game, &tokens[1], 1, seed).await,
    ];
    let tasks: Vec<_> = bots
        .into_iter()
        .map(|mut bot| {
            tokio::spawn(async move {
                bot.ready().await;
                bot.finish().await
            })
        })
        .collect();
    for task in tasks {
        let report = task.await.unwrap();
        assert_eq!(report.result, alone.result);
        assert_eq!(report.errors, []);
    }
    assert_eq!(owners(&pool, &game).await, [None, None]);
    // A signed-in user's list does not pick up games they never sat in.
    let ann = register(addr, "Ann", PASSWORD).await;
    assert_eq!(ann.games(addr).await.len(), 0);
}

#[sqlx::test(migrations = false)]
async fn seats_and_sessions_survive_a_restart(pool: PgPool) {
    let mut first = start(&pool).await;
    let addr = first.addr;
    let (ann, bob) = (
        register(addr, "Ann", PASSWORD).await,
        register(addr, "Bob", PASSWORD).await,
    );
    let (lobby, lobby_tokens) = ann.create_game(addr, &host_game()).await;
    let _ = through_the_link(addr, &lobby, &lobby_tokens[1], Some(&bob)).await;
    let (running, running_tokens) = bob.create_game(addr, &host_game()).await;
    let _ = through_the_link(addr, &running, &running_tokens[1], Some(&ann)).await;
    start_game(&first, &running).await;
    let before = (ann.games(addr).await, bob.games(addr).await);
    assert_eq!((before.0.len(), before.1.len()), (2, 2));

    first.shutdown();
    first.finished().await.unwrap();
    let second = start(&pool).await;
    let addr = second.addr;

    // Same cookies, same lists, same seats, no token needed.
    assert_eq!(ann.games(addr).await, before.0);
    assert_eq!(bob.games(addr).await, before.1);
    for (browser, game, seat) in [
        (&ann, &lobby, 0),
        (&bob, &lobby, 1),
        (&bob, &running, 0),
        (&ann, &running, 1),
    ] {
        let (mut client, got) = through_the_session(addr, game, browser).await;
        assert_eq!(got, seat, "{} in {game}", browser.username);
        if game == &running {
            // The resumed game is under way: a snapshot and the pending decision follow Welcome.
            assert!(matches!(
                client.message().await,
                ServerMessage::Snapshot { .. }
            ));
        }
    }
    // Claims are not repeated: the second server still refuses a third account.
    let cy = register(addr, "Cyd", PASSWORD).await;
    let (_, seat) = through_the_link(addr, &lobby, &lobby_tokens[1], Some(&cy)).await;
    assert_eq!(seat, 1);
    assert_eq!(
        owners(&pool, &lobby).await,
        [Some(ann.id.clone()), Some(bob.id.clone())]
    );
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_by_hand_ends_its_sessions_and_frees_its_seats_but_not_the_game(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let (game, tokens) = ann.create_game(addr, &host_game()).await;
    assert_eq!(owners(&pool, &game).await, [Some(ann.id.clone()), None]);

    sqlx::query("DELETE FROM users WHERE id = $1::uuid")
        .bind(&ann.id)
        .execute(&pool)
        .await
        .unwrap();
    // The seat is a guest seat again and the session went with the account.
    assert_eq!(owners(&pool, &game).await, [None, None]);
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
    let me = ann.get(addr, "/api/auth/me").await;
    assert_eq!(me.status, 401);
    let (status, _) = refused_at(addr, &format!("/ws/{game}"), &[("Cookie", &ann.cookie)]).await;
    assert_eq!(status, 401);
    // The game itself is untouched and its link works as for any guest.
    let (_socket, seat) = through_the_link(addr, &game, &tokens[0], None).await;
    assert_eq!(seat, 0);
}

#[sqlx::test(migrations = false)]
async fn a_game_that_was_never_recorded_cannot_be_claimed_but_its_links_work(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let ann = register(addr, "Ann", PASSWORD).await;
    let created = server
        .registry
        .register(common::table_setup(2, 1), vec!["A".into(), "B".into()])
        .await
        .unwrap();
    let game = created.id.to_string();
    let token = created.seats[0].1.expose().to_owned();

    let (_, seat) = through_the_link(addr, &game, &token, Some(&ann)).await;
    assert_eq!(seat, 0);
    assert_eq!(ann.games(addr).await.len(), 0);
    let (status, _) = refused_at(addr, &format!("/ws/{game}"), &[("Cookie", &ann.cookie)]).await;
    assert_eq!(status, 404);
}

/// An account made straight in the table, as far from the HTTP surface as it gets.
async fn insert_user(pool: &PgPool, name: &str) -> UserId {
    let id = UserId::random();
    sqlx::query(
        "INSERT INTO users (id, username, username_key, password_hash)
         VALUES ($1, $2, lower($2), 'not a real hash')",
    )
    .bind(id.uuid())
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = false)]
async fn binding_a_seat_has_five_outcomes_and_never_moves_a_bound_seat(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let registry = Registry::with_database(&bot_config(), pool.clone());
    let (ann, bob) = (
        insert_user(&pool, "ann").await,
        insert_user(&pool, "bob").await,
    );
    let requests: Vec<_> = ["mono-green", "mono-red"]
        .iter()
        .map(|id| mtg_server::registry::SeatRequest {
            name: (*id).to_owned(),
            deck: demo::find(id).unwrap(),
        })
        .collect();
    let created = registry.create_for(&requests, Some(ann)).await.unwrap();
    let game = created.id;
    let seat = PlayerId::from_index;

    // Seat 0 is Ann's from the start.
    assert_eq!(
        registry.bind_seat(game, seat(0), ann).await.unwrap(),
        Binding::Already
    );
    assert_eq!(
        registry.bind_seat(game, seat(0), bob).await.unwrap(),
        Binding::Taken
    );
    // Ann holds a seat here, so seat 1 is not hers; Bob may have it, once.
    assert_eq!(
        registry.bind_seat(game, seat(1), ann).await.unwrap(),
        Binding::Elsewhere
    );
    assert_eq!(
        registry.bind_seat(game, seat(1), bob).await.unwrap(),
        Binding::Bound
    );
    assert_eq!(
        registry.bind_seat(game, seat(1), bob).await.unwrap(),
        Binding::Already
    );
    assert_eq!(
        registry.bind_seat(game, seat(1), ann).await.unwrap(),
        Binding::Taken
    );
    // No such seat, no such game.
    assert_eq!(
        registry.bind_seat(game, seat(2), bob).await.unwrap(),
        Binding::Missing
    );
    let nowhere = GameId(Uuid::new_v4());
    assert_eq!(
        registry.bind_seat(nowhere, seat(0), bob).await.unwrap(),
        Binding::Missing
    );

    // The registry's own lookup agrees with the database.
    assert_eq!(registry.authenticate_user(game, ann).unwrap().seat, seat(0));
    assert_eq!(registry.authenticate_user(game, bob).unwrap().seat, seat(1));
    assert!(registry.authenticate_user(game, UserId::random()).is_none());
    assert!(registry.authenticate_user(nowhere, ann).is_none());
    // And so does the stored list.
    let listed = registry.games_of(bob).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].id, listed[0].seat), (game, 1));
    assert_eq!(
        listed[0]
            .seats
            .iter()
            .map(|s| s.user.as_deref())
            .collect::<Vec<_>>(),
        [Some("ann"), Some("bob")]
    );
}
