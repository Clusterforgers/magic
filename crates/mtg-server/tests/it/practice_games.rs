//! Goal: a learner has at most one practice game: leaving a lesson, restarting it, taking the next lesson and staying away past the grace each end it, a reload within the grace keeps it, the end call ends only practice games whose seat token it holds, and practice games are labelled in the admin's list and never in the public ones.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::time::{Duration, Instant};

use mtg_server::GameId;
use mtg_server::tutorial::PLAYER_SEAT;
use net::{TestServer, WsClient, windowed_config};
use rstest::rstest;
use serde_json::json;

/// The default grace: a practice game with no learner connected ends this long after the learner left.
const GRACE: Duration = Duration::from_mins(3);

/// A learner's seat in a practice game.
#[derive(Debug, Clone)]
struct Seat {
    game: String,
    token: String,
}

impl Seat {
    fn id(&self) -> GameId {
        self.game.parse().unwrap()
    }

    fn json(&self) -> serde_json::Value {
        json!({ "game": self.game, "token": self.token })
    }
}

/// Starts `lesson`, replacing `replaces` when given.
async fn start(server: &TestServer, lesson: &str, replaces: Option<&Seat>) -> Seat {
    let body = match replaces {
        Some(seat) => json!({ "lesson": lesson, "replaces": seat.json() }),
        None => json!({ "lesson": lesson }),
    };
    let reply = net::post_json(server.addr, "/api/tutorial", &body.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    let body = reply.json();
    Seat {
        game: body["game"].as_str().unwrap().to_owned(),
        token: body["token"].as_str().unwrap().to_owned(),
    }
}

/// Answers `POST /api/tutorial/end` for `seat`.
async fn end(server: &TestServer, seat: &Seat) -> u16 {
    let body = seat.json().to_string();
    net::post_json(server.addr, "/api/tutorial/end", &body)
        .await
        .status
}

/// The practice games in the registry.
fn practice(server: &TestServer) -> Vec<GameId> {
    let mut games = server.registry.practice_games();
    games.sort_by_key(ToString::to_string);
    games
}

/// Waits until the learner's seat of `seat` is connected (`true`) or not.
async fn learner_connected(server: &TestServer, seat: &Seat, connected: bool) {
    let handle = server.registry.handle(seat.id()).unwrap();
    for _ in 0..200 {
        let status = handle.status().await.unwrap();
        if status.connected.contains(&PLAYER_SEAT) == connected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the learner's seat never became connected = {connected}");
}

/// Opens the learner's socket and waits for the seat to count as connected.
async fn sit(server: &TestServer, seat: &Seat) -> WsClient {
    let mut client = net::connect(server.addr, &seat.game, &seat.token).await;
    client.join(&seat.game).await;
    client.welcome(&seat.game).await;
    learner_connected(server, seat, true).await;
    client
}

/// How a learner leaves a lesson.
#[derive(Debug, Clone, Copy)]
enum Way {
    /// Leave lesson, or Back to lessons: the end call.
    Leave,
    /// Restart lesson: the same lesson again, replacing this game.
    Restart,
    /// Next lesson, replacing this game.
    NextLesson,
    /// The tab closed and nobody came back within the grace.
    StayAway,
}

#[rstest]
#[case::leave(Way::Leave, 0)]
#[case::restart(Way::Restart, 1)]
#[case::next_lesson(Way::NextLesson, 1)]
#[case::stay_away(Way::StayAway, 0)]
#[tokio::test]
async fn every_way_out_of_a_lesson_ends_its_game(#[case] way: Way, #[case] left: usize) {
    let server = TestServer::start(windowed_config()).await;
    let first = start(&server, "mana", None).await;
    let mut client = sit(&server, &first).await;
    let base = Instant::now();
    assert_eq!(server.registry.sweep(base).await, 0);
    let then = match way {
        Way::Leave => {
            assert_eq!(end(&server, &first).await, 204);
            None
        }
        Way::Restart => Some(start(&server, "mana", Some(&first)).await),
        Way::NextLesson => Some(start(&server, "combat", Some(&first)).await),
        Way::StayAway => {
            client.close().await;
            learner_connected(&server, &first, false).await;
            assert_eq!(server.registry.sweep(base + GRACE / 2).await, 0);
            assert_eq!(practice(&server), [first.id()], "kept within the grace");
            assert_eq!(
                server
                    .registry
                    .sweep(base + GRACE + Duration::from_secs(1))
                    .await,
                1
            );
            None
        }
    };
    assert!(
        server.registry.handle(first.id()).is_none(),
        "{way:?} ended the game"
    );
    assert_eq!(practice(&server).len(), left);
    if let Some(then) = then {
        assert_eq!(practice(&server), [then.id()]);
    }
    if !matches!(way, Way::StayAway) {
        assert!(client.closed().await.is_some(), "the old socket is closed");
    }
    assert_eq!(
        end(&server, &first).await,
        404,
        "a second end finds nothing"
    );
}

#[tokio::test]
async fn a_reload_within_the_grace_keeps_the_game_and_a_connected_learner_keeps_it_however_long() {
    let server = TestServer::start(windowed_config()).await;
    let seat = start(&server, "stack", None).await;
    let mut client = sit(&server, &seat).await;
    let base = Instant::now();
    assert_eq!(server.registry.sweep(base).await, 0);
    client.close().await;
    learner_connected(&server, &seat, false).await;
    assert_eq!(
        server
            .registry
            .sweep(base + GRACE.saturating_sub(Duration::from_secs(10)))
            .await,
        0
    );
    let _back = sit(&server, &seat).await;
    for minutes in [5, 30, 120] {
        let now = base + Duration::from_mins(minutes);
        assert_eq!(server.registry.sweep(now).await, 0, "{minutes} min");
    }
    assert_eq!(practice(&server), [seat.id()]);
}

#[tokio::test]
async fn a_game_whose_learner_never_came_ends_after_the_grace() {
    let server = TestServer::start(windowed_config()).await;
    let _seat = start(&server, "seat", None).await;
    let base = Instant::now();
    assert_eq!(server.registry.sweep(base + GRACE / 2).await, 0);
    assert_eq!(
        server
            .registry
            .sweep(base + GRACE + Duration::from_secs(1))
            .await,
        1
    );
    assert_eq!(practice(&server), []);
}

#[tokio::test]
async fn the_end_call_ends_only_a_practice_game_whose_seat_token_it_holds() {
    let server = TestServer::start(windowed_config()).await;
    let seat = start(&server, "combat", None).await;
    let other = start(&server, "combat", None).await;
    let wrong = Seat {
        game: seat.game.clone(),
        token: other.token.clone(),
    };
    assert_eq!(end(&server, &wrong).await, 404);
    let unknown = Seat {
        game: "not-a-game".to_owned(),
        token: seat.token.clone(),
    };
    assert_eq!(end(&server, &unknown).await, 404);
    let (game, tokens) = net::create_game(server.addr, net::two_seats()).await;
    let real = Seat {
        game,
        token: tokens[0].clone(),
    };
    assert_eq!(end(&server, &real).await, 404, "a real game is never ended");
    assert!(server.registry.handle(real.id()).is_some());
    let replaced = start(&server, "combat", Some(&real)).await;
    assert!(server.registry.handle(real.id()).is_some(), "nor replaced");
    let reply = net::post_json(server.addr, "/api/tutorial/end", "{\"game\":1}").await;
    assert_eq!(reply.status, 400);
    let mut left = practice(&server);
    let mut want = vec![seat.id(), other.id(), replaced.id()];
    left.sort_by_key(ToString::to_string);
    want.sort_by_key(ToString::to_string);
    assert_eq!(
        left, want,
        "a guest without the token keeps the games it holds"
    );
}

#[tokio::test]
async fn practice_games_are_labelled_in_the_admin_list_and_absent_from_the_public_ones() {
    let server = TestServer::start(windowed_config()).await;
    let seat = start(&server, "turn", None).await;
    let _client = sit(&server, &seat).await;
    let rows = server.registry.admin_games().await;
    let row = rows.iter().find(|row| row.game == seat.game).unwrap();
    assert!(row.tutorial);
    assert_eq!(row.seats[1].name, mtg_server::tutorial::BOT_NAME);
    let lobbies = net::get(server.addr, "/api/lobbies").await;
    assert!(!lobbies.text().contains(&seat.game));
}
