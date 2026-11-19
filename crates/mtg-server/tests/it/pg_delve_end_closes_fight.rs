//! Goal: however a Delves run ends (the host ends it with hearts left in a fight lobby or a running fight, the party
//! loses its last heart, the party wins its fight and then ends the run, a migration closes it), no table of its fight
//! is left: the game is no lobby or running game of the hero's list and no row of the open games says "Your seat";
//! a restart ends a fight whose run no longer waits for it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::pg_delve_forfeit::{act, call, fight_run, game_of, read};
use crate::{net, pg_commander};

use mtg_server::{GameRecords, RecordStatus, ServerConfig};
use net::lobby;
use net::{Browser, PASSWORD, TestServer, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;

const CLOSE: &str = include_str!("../../migrations/20261118000001_delves_rework_closes_runs.sql");

/// How the run ends.
#[derive(Debug, Clone, Copy)]
enum End {
    /// The host ends it while the fight's table waits in its lobby.
    AbandonInLobby,
    /// The host ends it while the fight's game runs.
    AbandonRunning,
    /// The party retreats with its last heart.
    Wipe,
    /// The boss concedes, then the host ends the run at camp.
    WonThenAbandon,
}

async fn with_hooks(pool: &PgPool) -> TestServer {
    let config = ServerConfig {
        test_hooks: true,
        ..bot_config()
    };
    pg_commander::start_with(pool, config).await
}

async fn status_of(pool: &PgPool, game: &str) -> RecordStatus {
    GameRecords::new(pool.clone())
        .get(game.parse().unwrap())
        .await
        .unwrap()
        .unwrap()
        .status
}

/// Asserts that `who` is offered no way back into any of `games`: none is a lobby or running game of their list, and
/// no row of the open games marks them seated in one.
async fn nothing_to_return_to(server: &TestServer, who: &Browser, games: &[String]) {
    let mine = who.get(server.addr, "/api/me/games").await;
    assert_eq!(mine.status, 200, "{}", mine.text());
    let mine = mine.json();
    for listed in mine["games"].as_array().unwrap() {
        let open = listed["status"] == "lobby" || listed["status"] == "running";
        let ours = games.iter().any(|g| listed["game"] == g.as_str());
        assert!(!(open && ours), "still offered: {listed}");
    }
    let rows = who.get(server.addr, "/api/lobbies").await.json();
    for row in rows["games"].as_array().unwrap() {
        let ours = games.iter().any(|g| row["game"] == g.as_str());
        assert!(!(ours && row["seated"] == true), "still seated: {row}");
    }
}

/// Reads the run until `done` holds of it.
async fn until(server: &TestServer, id: &str, who: &Browser, done: impl Fn(&Value) -> bool) {
    for _ in 0..500 {
        if done(&read(server, id, who).await) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the run never reached the state waited for");
}

/// The ends, the run's status after each, and the host who plays it.
const ENDS: [(End, &str, &str); 4] = [
    (End::AbandonInLobby, "Abandoned", "lobbyhost"),
    (End::AbandonRunning, "Abandoned", "runninghost"),
    (End::Wipe, "Defeat", "wipehost"),
    (End::WonThenAbandon, "Abandoned", "winnerhost"),
];

#[sqlx::test(migrations = false)]
async fn an_ended_run_leaves_no_table(pool: PgPool) {
    let server = with_hooks(&pool).await;
    for (end, status, name) in ENDS {
        ends_cleanly(&server, &pool, end, status, name).await;
    }
}

/// Plays a run to its fight, ends it by `end` and checks that nothing of its tables is left.
async fn ends_cleanly(server: &TestServer, pool: &PgPool, end: End, status: &str, name: &str) {
    let host = register(server.addr, name, PASSWORD).await;
    let (id, game) = fight_run(server, &host, None).await;
    let mut games = vec![game.clone()];
    match end {
        End::AbandonInLobby => {
            let ended = act(server, &id, &host, json!({"kind": "abandon"})).await;
            assert_eq!(ended.status, 200, "{}", ended.text());
        }
        End::AbandonRunning => {
            lobby::start(&server.registry, &game, 3).await;
            let ended = act(server, &id, &host, json!({"kind": "abandon"})).await;
            assert_eq!(ended.status, 200, "{}", ended.text());
        }
        End::Wipe => {
            for _ in 0..3 {
                let view = act(server, &id, &host, json!({"kind": "retreat"})).await;
                assert_eq!(view.status, 200, "{}", view.text());
                if let Some(next) = view.json()["run"]["status"]["Fight"]["game"].as_str() {
                    games.push(next.to_owned());
                }
            }
        }
        End::WonThenAbandon => {
            lobby::start(&server.registry, &game, 3).await;
            let hook = format!("/api/test/delves/{id}/boss-concedes");
            let conceded = call(server, "POST", &hook, &host, None).await;
            assert_eq!(conceded.status, 204, "{}", conceded.text());
            until(server, &id, &host, |view| view["run"]["status"] == "Room").await;
            let ended = act(server, &id, &host, json!({"kind": "abandon"})).await;
            assert_eq!(ended.status, 200, "{}", ended.text());
        }
    }
    assert_eq!(read(server, &id, &host).await["run"]["status"], status);
    for game in &games {
        // A won table stays to show its result; any other is gone.
        match status_of(pool, game).await {
            RecordStatus::Finished => {}
            RecordStatus::Aborted => assert!(
                server.registry.handle(game.parse().unwrap()).is_none(),
                "{end:?}: the table of {game} is gone"
            ),
            other => panic!("{end:?}: the table of {game} is left {other:?}"),
        }
    }
    nothing_to_return_to(server, &host, &games).await;
}

#[sqlx::test(migrations = false)]
async fn a_restart_ends_the_fight_of_a_run_a_migration_closed(pool: PgPool) {
    let mut server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;
    // A run in its fight lobby that a migration closes, as the boss rework's did.
    sqlx::raw_sql(CLOSE).execute(&pool).await.unwrap();
    server.shutdown();
    server.finished().await.unwrap();

    let server =
        TestServer::start_with_cards(pool.clone(), bot_config(), pg_commander::source(&pool)).await;
    let view = read(&server, &id, &host).await;
    assert_eq!(view["run"]["status"], "Abandoned");
    assert!(server.registry.handle(game.parse().unwrap()).is_none());
    assert_eq!(status_of(&pool, &game).await, RecordStatus::Aborted);
    nothing_to_return_to(&server, &host, &[game]).await;
}

#[sqlx::test(migrations = false)]
async fn a_restart_keeps_the_fight_its_run_waits_for(pool: PgPool) {
    let mut server = pg_commander::start(&pool).await;
    let host = register(server.addr, "hostess", PASSWORD).await;
    let (id, game) = fight_run(&server, &host, None).await;
    server.shutdown();
    server.finished().await.unwrap();

    let server =
        TestServer::start_with_cards(pool.clone(), bot_config(), pg_commander::source(&pool)).await;
    assert!(server.registry.handle(game.parse().unwrap()).is_some());
    assert_eq!(game_of(&read(&server, &id, &host).await), game);
}
