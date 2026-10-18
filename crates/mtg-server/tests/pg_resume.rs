//! Goal: a server that restarts on the same database brings back every unfinished game with its seat tokens, so the game goes on to the very end an uninterrupted one reaches; finished games stay finished, lobbies survive, old lobbies and damaged games are retired without stopping the boot, and a token opens only its own seat.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use common::{Bot, drive, roomy};
use mtg_engine::replay;
use mtg_protocol::ClientMessage;
use mtg_server::config::Retention;
use mtg_server::registry::SeatRequest;
use mtg_server::{
    DeckRef, EventStore, GameId, GameRecipe, GameRecords, PgEventStore, RecordStatus, Registry,
    ResumeReport, ServerConfig, demo, migrate,
};
use mtg_testkit::{GameOutcome, play_game};
use net::bot::Stop;
use net::{TestServer, WsBot, bot_config};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

const DECKS: [&str; 4] = ["mono-green", "mono-red", "mono-white", "mono-green"];

fn requests(players: usize) -> Vec<SeatRequest> {
    DECKS[..players]
        .iter()
        .enumerate()
        .map(|(i, id)| SeatRequest {
            name: format!("Bot {i}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

fn recipe(players: usize, seed: u64) -> GameRecipe {
    let decks = DECKS[..players].iter().map(|id| DeckRef::Demo {
        id: (*id).to_owned(),
    });
    GameRecipe::normal(seed, decks)
}

/// The same game played straight on an engine with the same bots and seed.
fn straight(players: usize, seed: u64) -> GameOutcome {
    let setup = recipe(players, seed).build().unwrap();
    play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"))
}

fn config(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        game: roomy(),
        ..ServerConfig::default()
    }
}

/// A new process on the same database: a fresh registry that resumes what the last one left.
async fn reboot(pool: &PgPool, config: &ServerConfig) -> (Registry, ResumeReport) {
    let registry = Registry::with_database(config, pool.clone());
    let report = registry.resume().await.unwrap();
    (registry, report)
}

fn tokens(created: &mtg_server::registry::Created) -> Vec<String> {
    created
        .seats
        .iter()
        .map(|(_, token)| token.expose().to_owned())
        .collect()
}

/// Creates a game, seats a bot per player, readies them and plays `answers` answers.
async fn started(
    registry: &Registry,
    players: usize,
    seed: u64,
    answers: usize,
) -> (GameId, Vec<String>, Vec<Bot>) {
    let created = registry.create(&requests(players)).await.unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut bots = Vec::new();
    for seat in 0..players {
        bots.push(Bot::join(&handle, seat, seed).await);
    }
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &handle, answers).await, None);
    (created.id, tokens(&created), bots)
}

/// Reconnects every bot through its old token and returns the game's handle.
async fn come_back(
    registry: &Registry,
    game: GameId,
    tokens: &[String],
    bots: &mut [Bot],
) -> mtg_server::GameHandle {
    let mut handle = None;
    for (seat, token) in tokens.iter().enumerate() {
        let access = registry
            .authenticate(game, token)
            .expect("the old token still opens the game");
        assert_eq!((access.game, access.seat.index()), (game, seat));
        bots[seat].rejoin(&access.handle).await;
        handle = Some(access.handle);
    }
    handle.unwrap()
}

#[sqlx::test(migrations = false)]
async fn a_game_picks_up_after_a_restart_and_ends_as_if_nothing_had_happened(pool: PgPool) {
    migrate(&pool).await.unwrap();
    for (players, seed, answers_before) in [(2, 1, 1), (3, 2, 25), (4, 3, 200)] {
        let config = config(seed);
        let registry = Registry::with_database(&config, pool.clone());
        let (game, tokens, mut bots) = started(&registry, players, seed, answers_before).await;
        let stopped = registry.handle(game).unwrap().status().await.unwrap();
        drop(registry); // the process dies; the rows stay

        let (registry, report) = reboot(&pool, &config).await;
        assert_eq!(
            report,
            ResumeReport {
                resumed: 1,
                ..ResumeReport::default()
            }
        );
        let handle = come_back(&registry, game, &tokens, &mut bots).await;
        let resumed = handle.status().await.unwrap();
        assert!(resumed.started);
        assert_eq!(resumed.events, stopped.events);
        assert_eq!(resumed.state_hash, stopped.state_hash);
        assert_eq!(resumed.waiting_on, stopped.waiting_on);

        let result = drive(&mut bots, &handle, usize::MAX).await;
        let alone = straight(players, seed);
        assert_eq!(result, Some(alone.result), "{players} players");
        let logged = PgEventStore::new(pool.clone()).load(game).await.unwrap();
        assert_eq!(logged, alone.events);
        let setup = recipe(players, seed).build().unwrap();
        assert_eq!(
            replay(&setup, &logged).unwrap().state_hash(),
            alone.final_hash
        );
        for bot in &bots {
            assert_eq!(bot.errors, []);
        }

        let record = GameRecords::new(pool.clone()).get(game).await.unwrap();
        let record = record.unwrap();
        assert_eq!(record.status, RecordStatus::Finished);
        assert_eq!(record.result, Some(alone.result));
        assert!(record.seats.iter().all(|seat| seat.joined));
    }
}

fn seeded(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        ..bot_config()
    }
}

#[sqlx::test(migrations = false)]
async fn sockets_with_the_old_tokens_land_in_the_same_game_after_a_restart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let seed = 3;
    let mut server = TestServer::start_on(pool.clone(), seeded(seed)).await;
    let seats: Vec<Value> = (0..2)
        .map(|i| json!({ "name": format!("Bot {i}"), "deck": DECKS[i] }))
        .collect();
    let (game, tokens) =
        net::create_game(server.addr, json!({ "format": "normal", "seats": seats })).await;
    let id: GameId = game.parse().unwrap();

    let mut victim = WsBot::connect(server.addr, &game, &tokens[0], 0, seed).await;
    let other = WsBot::connect(server.addr, &game, &tokens[1], 1, seed).await;
    victim.pause_at = Some(30);
    let other = tokio::spawn(async move {
        let mut other = other;
        other.ready().await;
        other.play().await
    });
    victim.ready().await;
    assert_eq!(victim.play().await, Stop::Paused);

    // The process dies mid-game: sockets and server go, the database stays.
    other.abort();
    victim.client.hang_up();
    server.shutdown();
    server.finished().await.unwrap();
    drop(server);

    let mut server = TestServer::start_on(pool.clone(), seeded(seed)).await;
    assert!(server.registry.handle(id).is_some(), "the game came back");
    let before = net::get(server.addr, &format!("/api/games/{game}")).await;
    assert_eq!(before.status, 200);
    assert_eq!(before.json()["started"], true);

    // Each seat returns with the token it was given before the restart; the held answer meets the
    // very decision it was made for.
    let _ = victim.reconnect(server.addr, &game, &tokens[0]).await;
    let fresh = WsBot::connect(server.addr, &game, &tokens[1], 1, seed).await;
    let (victim, fresh) = tokio::join!(victim.finish(), fresh.finish());
    assert_eq!(victim.result, fresh.result);
    assert_eq!((victim.errors, fresh.errors), (vec![], vec![]));

    let view = net::get(server.addr, &format!("/api/games/{game}"))
        .await
        .json();
    assert_eq!(view["result"], serde_json::to_value(victim.result).unwrap());
    let logged = PgEventStore::new(pool.clone()).load(id).await.unwrap();
    let setup = recipe(2, seed).build().unwrap();
    assert!(replay(&setup, &logged).unwrap().result().is_some());
    let record = GameRecords::new(pool).get(id).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Finished);
    server.shutdown();
    server.finished().await.unwrap();
}

#[sqlx::test(migrations = false)]
async fn finished_games_are_not_resumed_and_a_lagging_record_is_corrected(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let (seed, config) = (5, config(5));
    let registry = Registry::with_database(&config, pool.clone());
    let game = registry.create(&requests(2)).await.unwrap().id;
    let handle = registry.handle(game).unwrap();
    let result = common::play_table(&handle, 2, seed).await[0].result;
    drop((handle, registry));

    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(report, ResumeReport::default());
    assert!(registry.is_empty());
    assert!(registry.handle(game).is_none());
    let records = GameRecords::new(pool.clone());
    assert_eq!(
        records.get(game).await.unwrap().unwrap().status,
        RecordStatus::Finished
    );

    // The process died after the last event was saved but before the record heard of it.
    sqlx::query("UPDATE games SET status = 'running', result = NULL, finished_at = NULL")
        .execute(&pool)
        .await
        .unwrap();
    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(
        report,
        ResumeReport {
            finished: 1,
            ..ResumeReport::default()
        }
    );
    assert!(registry.is_empty(), "an ended game is not loaded");
    let record = records.get(game).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Finished);
    assert_eq!(record.result, Some(result));
}

#[sqlx::test(migrations = false)]
async fn lobbies_survive_a_restart_and_start_afterwards(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let (seed, config) = (6, config(6));
    let registry = Registry::with_database(&config, pool.clone());
    let quiet = registry.create(&requests(2)).await.unwrap();
    let lively = registry.create(&requests(2)).await.unwrap();
    let bot = Bot::join(&registry.handle(lively.id).unwrap(), 1, seed).await;
    registry.handle(lively.id).unwrap().status().await.unwrap();
    drop((registry, bot));

    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(
        report,
        ResumeReport {
            resumed: 2,
            lobbies: 2,
            ..ResumeReport::default()
        }
    );
    for created in [&quiet, &lively] {
        let described = registry.describe(created.id).await.unwrap();
        assert!(!described.started);
        let names: Vec<_> = described.seats.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Bot 0", "Bot 1"]);
        assert_eq!(described.connected, []);
    }
    let records = GameRecords::new(pool.clone());
    let joined = |game: GameId| {
        let records = records.clone();
        async move {
            let record = records.get(game).await.unwrap().unwrap();
            record.seats.iter().map(|s| s.joined).collect::<Vec<_>>()
        }
    };
    assert_eq!(joined(quiet.id).await, [false, false]);
    assert_eq!(joined(lively.id).await, [false, true]);

    // The old tokens seat the players again and the game then runs to the end.
    let tokens = tokens(&lively);
    let mut bots = Vec::new();
    for (seat, token) in tokens.iter().enumerate() {
        let access = registry.authenticate(lively.id, token).unwrap();
        bots.push(Bot::join(&access.handle, seat, seed).await);
    }
    let handle = registry.handle(lively.id).unwrap();
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert!(handle.status().await.unwrap().started);
    let result = drive(&mut bots, &handle, usize::MAX).await;
    assert_eq!(result, Some(straight(2, seed).result));
    assert_eq!(joined(lively.id).await, [true, true]);
    // The quiet lobby is untouched by all of it.
    assert_eq!(
        records.get(quiet.id).await.unwrap().unwrap().status,
        RecordStatus::Lobby
    );
}

#[sqlx::test(migrations = false)]
async fn a_lobby_older_than_the_retention_rule_is_retired_but_an_old_running_game_is_not(
    pool: PgPool,
) {
    migrate(&pool).await.unwrap();
    let config = config(7);
    let registry = Registry::with_database(&config, pool.clone());
    let stale = registry.create(&requests(2)).await.unwrap().id;
    let (running, _, _) = started(&registry, 2, 7, 3).await;
    let fresh = registry.create(&requests(2)).await.unwrap().id;
    drop(registry);
    let old = Retention::default().lobby_after + Duration::from_secs(60);
    for game in [stale, running] {
        sqlx::query(
            "UPDATE games SET created_at = now() - make_interval(secs => $2) WHERE id = $1",
        )
        .bind(game.uuid())
        .bind(old.as_secs_f64())
        .execute(&pool)
        .await
        .unwrap();
    }

    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(
        report,
        ResumeReport {
            resumed: 2,
            lobbies: 1,
            expired: 1,
            ..ResumeReport::default()
        }
    );
    assert!(registry.handle(stale).is_none());
    assert!(registry.handle(running).is_some());
    assert!(registry.handle(fresh).is_some());
    let status = |game: GameId| {
        let records = GameRecords::new(pool.clone());
        async move { records.get(game).await.unwrap().unwrap().status }
    };
    assert_eq!(status(stale).await, RecordStatus::Expired);
    assert_eq!(status(running).await, RecordStatus::Running);
    // Retired for good: the next boot does not look at it again.
    let (_, report) = reboot(&pool, &config).await;
    assert_eq!((report.resumed, report.expired), (2, 0));
}

#[sqlx::test(migrations = false)]
async fn a_game_the_janitor_dropped_does_not_come_back(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let config = config(8);
    let registry = Registry::with_database(&config, pool.clone());
    let lobby = registry.create(&requests(2)).await.unwrap().id;
    let (abandoned, _, bots) = started(&registry, 2, 8, 5).await;
    drop(bots);
    registry
        .handle(abandoned)
        .unwrap()
        .leave(common::seat(0))
        .await;
    registry
        .handle(abandoned)
        .unwrap()
        .leave(common::seat(1))
        .await;
    let ended = registry.create(&requests(2)).await.unwrap().id;
    common::play_table(&registry.handle(ended).unwrap(), 2, 9).await;

    // The lobby and the abandoned game are due at once; the finished one a sweep after it was seen.
    let later = Instant::now() + Duration::from_hours(48);
    assert_eq!(registry.sweep(later).await, 2);
    assert_eq!(registry.sweep(later + Duration::from_mins(11)).await, 1);
    assert!(registry.is_empty());

    let records = GameRecords::new(pool.clone());
    let status = |game: GameId| {
        let records = records.clone();
        async move { records.get(game).await.unwrap().unwrap().status }
    };
    assert_eq!(status(lobby).await, RecordStatus::Expired);
    assert_eq!(status(abandoned).await, RecordStatus::Expired);
    assert_eq!(status(ended).await, RecordStatus::Finished);
    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(report, ResumeReport::default());
    assert!(registry.is_empty());
}

fn digest(text: &str) -> Vec<u8> {
    Sha256::digest(text.as_bytes()).to_vec()
}

/// Writes a game row and seat rows by hand, as a damaged database or another version might hold them.
async fn plant(pool: &PgPool, id: Uuid, recipe: &Value, seat_numbers: &[i32]) {
    sqlx::query("INSERT INTO games (id, status, recipe) VALUES ($1, 'lobby', $2)")
        .bind(id)
        .bind(recipe)
        .execute(pool)
        .await
        .unwrap();
    for seat in seat_numbers {
        sqlx::query(
            "INSERT INTO seats (game_id, seat, name, token_digest) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(seat)
        .bind(format!("Seat {seat}"))
        .bind(digest(&format!("{id}/{seat}")))
        .execute(pool)
        .await
        .unwrap();
    }
}

fn decks(ids: &[&str]) -> Value {
    json!(
        ids.iter()
            .map(|id| json!({ "kind": "demo", "id": id }))
            .collect::<Vec<_>>()
    )
}

#[sqlx::test(migrations = false)]
async fn games_with_damaged_records_are_marked_failed_and_the_boot_goes_on(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let (seed, config) = (11, config(11));

    // Games a person could have made, then broken in the database.
    let registry = Registry::with_database(&config, pool.clone());
    let (good, good_tokens, mut bots) = started(&registry, 2, seed, 20).await;
    let (undecodable, ..) = started(&registry, 2, seed, 20).await;
    let (gap, ..) = started(&registry, 2, seed, 20).await;
    let (other_game, ..) = started(&registry, 2, seed, 20).await;
    drop(registry);
    sqlx::query(
        "UPDATE game_events SET event = '{\"Nonsense\": 1}'::jsonb WHERE game_id = $1 AND seq = 2",
    )
    .bind(undecodable.uuid())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM game_events WHERE game_id = $1 AND seq = 2")
        .bind(gap.uuid())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE game_events SET event = '{\"FirstPlayerChosen\": {\"player\": 99}}'::jsonb
         WHERE game_id = $1 AND seq = 2",
    )
    .bind(other_game.uuid())
    .execute(&pool)
    .await
    .unwrap();

    // Records that never described a game, with what the failure should mention.
    let valid = json!({ "format": "normal", "seed": 1, "decks": decks(&DECKS[..2]) });
    let planted: Vec<(&str, GameId, &str)> = {
        let cases: Vec<(&str, Value, Vec<i32>, &str)> = vec![
            (
                "not an object",
                json!("nope"),
                vec![0, 1],
                "recipe does not decode",
            ),
            (
                "unknown format",
                json!({ "format": "commander", "seed": 1, "decks": decks(&DECKS[..2]) }),
                vec![0, 1],
                "recipe does not decode",
            ),
            (
                "unknown deck",
                json!({ "format": "normal", "seed": 1, "decks": decks(&["mono-green", "mono-purple"]) }),
                vec![0, 1],
                "mono-purple",
            ),
            (
                "one deck",
                json!({ "format": "normal", "seed": 1, "decks": decks(&["mono-green"]) }),
                vec![0],
                "recipe does not build",
            ),
            ("too few seat rows", valid.clone(), vec![0], "seats"),
            ("too many seat rows", valid.clone(), vec![0, 1, 2], "seats"),
            ("a hole in the seats", valid.clone(), vec![0, 2], "seats"),
            ("no seat rows", valid, vec![], "seats"),
        ];
        let mut planted = Vec::new();
        for (n, (name, recipe, seats, needle)) in cases.into_iter().enumerate() {
            let id = Uuid::from_u128(0x1000 + n as u128);
            plant(&pool, id, &recipe, &seats).await;
            planted.push((name, GameId(id), needle));
        }
        planted
    };

    let (registry, report) = reboot(&pool, &config).await;
    let mut expected_failed: Vec<GameId> = [undecodable, gap, other_game]
        .into_iter()
        .chain(planted.iter().map(|(_, id, _)| *id))
        .collect();
    expected_failed.sort();
    let mut failed = report.failed.clone();
    failed.sort();
    assert_eq!(failed, expected_failed, "{report:?}");
    assert_eq!((report.resumed, report.lobbies), (1, 0));

    let records = GameRecords::new(pool.clone());
    for (name, id, needle) in &planted {
        let record = records.get(*id).await.unwrap().unwrap();
        assert_eq!(record.status, RecordStatus::Failed, "{name}");
        let reason = record.failure.unwrap_or_default();
        assert!(reason.contains(needle), "{name}: {reason}");
        assert!(registry.handle(*id).is_none(), "{name}");
    }
    for (name, id) in [
        ("an event that does not decode", undecodable),
        ("a gap in the log", gap),
        ("an event that cannot be applied", other_game),
    ] {
        let record = records.get(id).await.unwrap().unwrap();
        assert_eq!(record.status, RecordStatus::Failed, "{name}");
        assert!(!record.failure.unwrap_or_default().is_empty(), "{name}");
        assert!(registry.handle(id).is_none(), "{name}");
    }

    // The healthy game is untouched and plays on; the failed ones are not tried again.
    let handle = come_back(&registry, good, &good_tokens, &mut bots).await;
    let result = drive(&mut bots, &handle, usize::MAX).await;
    assert_eq!(result, Some(straight(2, seed).result));
    let (_, second) = reboot(&pool, &config).await;
    assert_eq!(second, ResumeReport::default());
}

#[sqlx::test(migrations = false)]
async fn a_token_opens_only_its_own_seat_of_its_own_game_before_and_after_a_restart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let config = config(12);
    let registry = Registry::with_database(&config, pool.clone());
    let a = registry.create(&requests(2)).await.unwrap();
    let b = registry.create(&requests(3)).await.unwrap();
    let games = [(a.id, tokens(&a)), (b.id, tokens(&b))];
    drop(registry);

    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(report.resumed, 2);
    let unknown = GameId(Uuid::from_u128(99));
    for (game, own) in &games {
        for (seat, token) in own.iter().enumerate() {
            for (other, _) in &games {
                match registry.authenticate(*other, token) {
                    Some(access) => assert_eq!((*other, access.seat.index()), (*game, seat)),
                    None => assert_ne!(other, game, "a token must open its own game"),
                }
            }
            assert!(registry.authenticate(unknown, token).is_none());
        }
    }
    // A digest of a token is not a token.
    let digest_hex = digest(&games[0].1[0])
        .iter()
        .fold(String::new(), |mut hex, byte| {
            write!(hex, "{byte:02x}").unwrap();
            hex
        });
    assert!(registry.authenticate(a.id, &digest_hex).is_none());
}

#[tokio::test]
async fn a_server_without_a_database_has_nothing_to_resume() {
    let registry = Registry::new(&config(1));
    assert_eq!(registry.resume().await.unwrap(), ResumeReport::default());
}
