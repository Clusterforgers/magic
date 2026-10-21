//! Goal: a game that failed to resume is listed by `GET /api/me/games` for each account that holds a seat in it, as failed and with a reason in plain words for every way a game can fail (a changed deck in a lobby and under way, a recipe that cannot be read, a deck the server lost, a missing seat, a log that cannot be replayed, a seat nobody holds), never with the operator's detail; nobody else reads the reason, not a stranger, not a guest with a seat link, not a visitor; a player removes the game from their own list and from nobody else's, only a failed game can be removed, and a removal outlives a restart.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use mtg_core::ids::PlayerId;
use mtg_protocol::ClientMessage;
use mtg_server::{GameId, GameRecords, RecordStatus, ServerConfig, migrate};
use net::{Browser, PASSWORD, TestServer, bot_config, get, refused_at, register};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A fingerprint no deck has: a seat whose stored one is this has a deck that changed.
const NO_DECK: &str = "v1:0000000000000000000000000000000000000000000000000000000000000000";

fn config() -> ServerConfig {
    let mut config = bot_config();
    config.fixed_seed = Some(31);
    config.auth.limits.login_per_ip = 10_000;
    config.auth.limits.login_per_username = 10_000;
    config.auth.limits.register_per_ip = 10_000;
    config
}

async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    TestServer::start_on(pool.clone(), config()).await
}

/// Stops the server and starts a new process on the same database, which resumes what is stored.
async fn restart(server: TestServer, pool: &PgPool) -> TestServer {
    restart_after(server, pool, &[]).await
}

/// [`restart`], with `statements` run on `game` while no server is up.
async fn restart_after(
    mut server: TestServer,
    pool: &PgPool,
    changes: &[(&str, &[&'static str])],
) -> TestServer {
    server.shutdown();
    server.finished().await.unwrap();
    drop(server);
    for (game, statements) in changes {
        plant(pool, game, statements).await;
    }
    TestServer::start_on(pool.clone(), config()).await
}

/// A deck of the second seat that is not what it was.
const DECK_CHANGED: &[&str] = &[
    "UPDATE games SET recipe = jsonb_set(recipe, '{fingerprints,1}', to_jsonb($2::text)) WHERE id = $1",
];

fn two_seats() -> Value {
    json!({ "format": "normal", "seats": [
        { "name": "Host", "deck": "mono-green" },
        { "name": "Friend", "deck": "mono-red" },
    ]})
}

/// Presses Ready for both seats, so the game starts and has a log.
async fn begin(server: &TestServer, game: &str) {
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    for seat in 0..2 {
        let ready = ClientMessage::Ready;
        let seat = PlayerId::from_index(seat);
        handle.send(seat, ready).await.unwrap();
    }
    assert!(handle.status().await.unwrap().started);
}

/// Runs statements against a game: `$1` is its id and `$2` a fingerprint no deck has.
async fn plant(pool: &PgPool, game: &str, statements: &[&'static str]) {
    for statement in statements {
        sqlx::query(*statement)
            .bind(game.parse::<uuid::Uuid>().unwrap())
            .bind(NO_DECK)
            .execute(pool)
            .await
            .unwrap();
    }
}

/// Binds an account to a seat of a game, as taking the seat's link while signed in does.
async fn seat_at(pool: &PgPool, game: &str, seat: i32, user: &Browser) {
    sqlx::query("UPDATE seats SET user_id = $3::uuid WHERE game_id = $1::uuid AND seat = $2")
        .bind(game)
        .bind(seat)
        .bind(&user.id)
        .execute(pool)
        .await
        .unwrap();
}

/// The game's entry in the browser's list, if it has one.
async fn entry(browser: &Browser, server: &TestServer, game: &str) -> Option<Value> {
    let games = browser.games(server.addr).await;
    games.into_iter().find(|g| g["game"] == game)
}

async fn record(pool: &PgPool, game: &str) -> mtg_server::GameRecord {
    let id: GameId = game.parse().unwrap();
    GameRecords::new(pool.clone())
        .get(id)
        .await
        .unwrap()
        .unwrap()
}

/// How a game was broken: a name, whether it had started, what to run, the reason its players
/// read and a piece of the operator's detail.
struct Breakage {
    name: &'static str,
    started: bool,
    statements: &'static [&'static str],
    reason: &'static str,
    detail: &'static str,
}

const BREAKAGES: &[Breakage] = &[
    Breakage {
        name: "a deck changed in a lobby",
        started: false,
        statements: &[
            "UPDATE games SET recipe = jsonb_set(recipe, '{fingerprints,1}', to_jsonb($2::text)) WHERE id = $1",
        ],
        reason: "The deck of Friend has changed since this game was made, so it cannot continue.",
        detail: "the deck changed: seat 1",
    },
    Breakage {
        name: "a deck changed in a game under way",
        started: true,
        statements: &[
            "UPDATE games SET recipe = jsonb_set(recipe, '{fingerprints,0}', to_jsonb($2::text)) WHERE id = $1",
        ],
        reason: "The deck of Host has changed since this game was made, so it cannot continue.",
        detail: "the deck changed: seat 0",
    },
    Breakage {
        name: "a recipe that cannot be read",
        started: false,
        statements: &["UPDATE games SET recipe = '\"nope\"'::jsonb WHERE id = $1"],
        reason: "The saved setup of this game cannot be read, so it cannot continue.",
        detail: "recipe does not decode",
    },
    Breakage {
        name: "a deck the server no longer has",
        started: false,
        statements: &[
            "UPDATE games SET recipe = jsonb_set(recipe, '{decks,1}', '{\"kind\":\"demo\",\"id\":\"mono-purple\"}'::jsonb) WHERE id = $1",
        ],
        reason: "The cards of this game's decks are not available, so it cannot continue.",
        detail: "mono-purple",
    },
    Breakage {
        name: "a seat row that is gone",
        started: false,
        statements: &["DELETE FROM seats WHERE game_id = $1 AND seat = 1"],
        reason: "The saved seats of this game do not fit its setup, so it cannot continue.",
        detail: "records hold seats",
    },
    Breakage {
        name: "a log that cannot be replayed",
        started: true,
        statements: &[
            "UPDATE game_events SET event = '{\"Nonsense\": 1}'::jsonb WHERE game_id = $1 AND seq = 2",
        ],
        reason: "The saved moves of this game cannot be loaded, so it cannot continue.",
        detail: "",
    },
    Breakage {
        name: "a game under way with a seat nobody holds",
        started: true,
        statements: &[
            "UPDATE seats SET state = 'open', token_digest = NULL, user_id = NULL, name = '' WHERE game_id = $1 AND seat = 1",
        ],
        reason: "This game began with a seat nobody holds, so it cannot continue.",
        detail: "seat nobody holds",
    },
];

/// Words that belong to the server's insides and so never to a reason.
const INTERNALS: &[&str] = &[
    "recipe",
    "fingerprint",
    "v1:",
    "oracle",
    "panic",
    "seq",
    "Nonsense",
];

#[sqlx::test(migrations = false)]
async fn every_way_a_game_fails_is_listed_as_failed_with_a_plain_reason(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;

    let mut broken = Vec::new();
    for breakage in BREAKAGES {
        let (game, _) = ann.create_game(server.addr, &two_seats()).await;
        if breakage.started {
            begin(&server, &game).await;
        }
        broken.push((breakage, game));
    }
    let (healthy, _) = ann.create_game(server.addr, &two_seats()).await;
    let (running, _) = ann.create_game(server.addr, &two_seats()).await;
    begin(&server, &running).await;
    let changes: Vec<(&str, &[&'static str])> = broken
        .iter()
        .map(|(breakage, game)| (game.as_str(), breakage.statements))
        .collect();
    let server = restart_after(server, &pool, &changes).await;
    let addr = server.addr;

    for (breakage, game) in &broken {
        let name = breakage.name;
        let listed = entry(&ann, &server, game)
            .await
            .unwrap_or_else(|| panic!("{name}: not listed"));
        assert_eq!(listed["status"], "failed", "{name}");
        assert_eq!(listed["reason"], breakage.reason, "{name}");
        assert_eq!(listed["result"], Value::Null, "{name}");
        assert_eq!(listed["seat"], 0, "{name}");
        assert_eq!(listed["name"], "Host", "{name}");

        // The words are for people: no id, no stored text, none of the server's vocabulary.
        let reason = listed["reason"].as_str().unwrap();
        assert!(!reason.contains(game.as_str()), "{name}: {reason}");
        for internal in INTERNALS {
            assert!(!reason.contains(internal), "{name}: {reason}");
        }

        // The record keeps the operator's detail apart from the sentence.
        let stored = record(&pool, game).await;
        assert_eq!(stored.status, RecordStatus::Failed, "{name}");
        assert_eq!(
            stored.failure_reason.as_deref(),
            Some(breakage.reason),
            "{name}"
        );
        let detail = stored.failure.unwrap_or_default();
        assert!(detail.contains(breakage.detail), "{name}: {detail}");
        assert_ne!(detail, breakage.reason, "{name}");

        // The game is gone for everything but the list.
        assert_eq!(
            get(addr, &format!("/api/games/{game}")).await.status,
            404,
            "{name}"
        );
    }

    // A game that resumed is listed as before, with no reason.
    assert_eq!(
        entry(&ann, &server, &healthy).await.unwrap()["status"],
        "lobby"
    );
    assert_eq!(
        entry(&ann, &server, &healthy).await.unwrap()["reason"],
        Value::Null
    );
    let live = entry(&ann, &server, &running).await.unwrap();
    assert_eq!(
        (&live["status"], &live["reason"]),
        (&json!("running"), &Value::Null)
    );
}

#[sqlx::test(migrations = false)]
async fn a_game_marked_failed_before_reasons_were_kept_says_so_plainly(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let (game, _) = ann.create_game(server.addr, &two_seats()).await;
    plant(
        &pool,
        &game,
        &["UPDATE games SET status = 'failed', failure = 'the recipe does not decode: v1' WHERE id = $1"],
    )
    .await;

    let listed = entry(&ann, &server, &game).await.unwrap();
    assert_eq!(listed["status"], "failed");
    assert_eq!(
        listed["reason"],
        "This game could not be restored, so it cannot continue."
    );
}

#[sqlx::test(migrations = false)]
async fn only_the_players_of_a_failed_game_read_its_reason(pool: PgPool) {
    let server = start(&pool).await;
    let addr = server.addr;
    let (ann, bob, cy) = (
        register(addr, "Ann", PASSWORD).await,
        register(addr, "Bob", PASSWORD).await,
        register(addr, "Cyrus", PASSWORD).await,
    );
    let (game, tokens) = ann.create_game(addr, &two_seats()).await;
    seat_at(&pool, &game, 1, &bob).await;
    let (elsewhere, _) = cy.create_game(addr, &two_seats()).await;
    let server = restart_after(server, &pool, &[(&game, DECK_CHANGED)]).await;
    let addr = server.addr;

    // Both players hold a seat and read it, each from their own seat.
    let anns = entry(&ann, &server, &game).await.unwrap();
    let bobs = entry(&bob, &server, &game).await.unwrap();
    assert_eq!(
        (&anns["status"], &bobs["status"]),
        (&json!("failed"), &json!("failed"))
    );
    assert_eq!(anns["reason"], bobs["reason"]);
    assert_eq!(
        (anns["seat"].as_i64(), bobs["seat"].as_i64()),
        (Some(0), Some(1))
    );

    // A stranger's list holds only their own game, and nothing of this one.
    let strangers = cy.get(addr, "/api/me/games").await;
    assert_eq!(strangers.status, 200);
    let body = strangers.json();
    let ids: Vec<&str> = body["games"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["game"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [elsewhere.as_str()]);
    assert!(!strangers.text().contains("has changed"));

    // A visitor has no list at all.
    let visitor = get(addr, "/api/me/games").await;
    assert_eq!(visitor.status, 401);
    assert!(!visitor.text().contains(&game));

    // Every other way in answers as for a game that never existed.
    let never = get(addr, "/api/games/00000000-0000-4000-8000-000000000000").await;
    let lookup = get(addr, &format!("/api/games/{game}")).await;
    assert_eq!((lookup.status, lookup.text()), (404, never.text()));
    for token in &tokens {
        let (status, body) = refused_at(addr, &format!("/ws/{game}/{token}"), &[]).await;
        assert_eq!(status, 404);
        assert!(!body.contains("has changed"));
    }
    let (status, body) = refused_at(addr, &format!("/ws/{game}"), &[("Cookie", &cy.cookie)]).await;
    assert_eq!(status, 404);
    assert!(!body.contains("has changed"));
    let lobbies = get(addr, "/api/lobbies").await;
    assert!(!lobbies.text().contains(&game));

    // Nor can a stranger take it off the players' lists.
    assert_eq!(
        cy.delete(addr, &format!("/api/me/games/{game}"))
            .await
            .status,
        404
    );
    assert!(entry(&ann, &server, &game).await.is_some());
}

#[sqlx::test(migrations = false)]
async fn a_player_removes_a_failed_game_from_their_own_list_and_nobody_elses(pool: PgPool) {
    let server = start(&pool).await;
    let (ann, bob) = (
        register(server.addr, "Ann", PASSWORD).await,
        register(server.addr, "Bob", PASSWORD).await,
    );
    let (game, _) = ann.create_game(server.addr, &two_seats()).await;
    seat_at(&pool, &game, 1, &bob).await;
    let (lobby, _) = ann.create_game(server.addr, &two_seats()).await;
    let (running, _) = ann.create_game(server.addr, &two_seats()).await;
    begin(&server, &running).await;
    let server = restart_after(server, &pool, &[(&game, DECK_CHANGED)]).await;
    let addr = server.addr;
    let path = format!("/api/me/games/{game}");

    // Ann removes it: 204, gone from her list, still on Bob's, and the record keeps her seat.
    let removed = ann.delete(addr, &path).await;
    assert_eq!((removed.status, removed.text()), (204, String::new()));
    assert!(entry(&ann, &server, &game).await.is_none());
    assert!(entry(&bob, &server, &game).await.is_some());
    let bound: Option<String> =
        sqlx::query_scalar("SELECT user_id::text FROM seats WHERE game_id = $1::uuid AND seat = 0")
            .bind(&game)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(bound.as_deref(), Some(ann.id.as_str()));
    assert_eq!(record(&pool, &game).await.status, RecordStatus::Failed);

    // A second removal finds nothing to remove.
    let again = ann.delete(addr, &path).await;
    assert_eq!(
        (again.status, again.error_code().as_str()),
        (404, "not_found")
    );

    // Only a failed game can be removed: the others stay, whatever their state.
    for other in [&lobby, &running] {
        let reply = ann.delete(addr, &format!("/api/me/games/{other}")).await;
        assert_eq!(reply.status, 404, "{}", reply.text());
        assert!(entry(&ann, &server, other).await.is_some());
    }
    let nonsense = ann.delete(addr, "/api/me/games/not-a-game").await;
    assert_eq!(nonsense.status, 404);

    // The request needs a session and a page of this site.
    let visitor = net::request(addr, "DELETE", &path, &[], b"").await;
    assert_eq!(
        (visitor.status, visitor.error_code().as_str()),
        (401, "unauthenticated")
    );
    let across = bob
        .send(
            addr,
            "DELETE",
            &path,
            "",
            &[("Origin", "https://elsewhere.example")],
        )
        .await;
    assert_eq!(
        (across.status, across.error_code().as_str()),
        (403, "forbidden_origin")
    );
    assert!(entry(&bob, &server, &game).await.is_some());

    // Bob removes his own entry; the removal outlives a restart, which does not try the game again.
    assert_eq!(bob.delete(addr, &path).await.status, 204);
    let server = restart(server, &pool).await;
    assert!(entry(&ann, &server, &game).await.is_none());
    assert!(entry(&bob, &server, &game).await.is_none());
    assert!(entry(&ann, &server, &lobby).await.is_some());
    assert_eq!(record(&pool, &game).await.status, RecordStatus::Failed);

    // An operator who repairs the game by hand finds both seats bound as they were.
    plant(
        &pool,
        &game,
        &["UPDATE games SET status = 'lobby' WHERE id = $1"],
    )
    .await;
    let revived = entry(&ann, &server, &game).await.unwrap();
    assert_eq!(
        (&revived["status"], &revived["reason"]),
        (&json!("lobby"), &Value::Null)
    );
}
