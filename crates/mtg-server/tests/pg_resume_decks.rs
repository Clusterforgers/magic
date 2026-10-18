//! Goal: a restarted server resumes a game only while its decks still hash to what the game was created with: an unchanged deck resumes with its log, a changed card, list order, list size or seat fails the game with "the deck changed" and the seat (in the record and in the log) before anything is replayed, the failed game is then gone for tokens, lookups, sockets and "My games" and can be revived by hand once the deck is right, and a recipe stored before fingerprints existed resumes unchecked with a warning.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::io;
use std::sync::{Arc, Mutex, OnceLock};

use common::{Bot, drive, roomy};
use mtg_core::card::{CardDefinition, CardType};
use mtg_core::mana::ManaCost;
use mtg_protocol::ClientMessage;
use mtg_server::registry::SeatRequest;
use mtg_server::{
    DeckFingerprint, GameId, GameRecords, RecordStatus, Registry, ResumeReport, ServerConfig, demo,
    migrate,
};
use net::{PASSWORD, TestServer, bot_config, refused_at, register};
use serde_json::{Value, json};
use sqlx::PgPool;

type Deck = Vec<Arc<CardDefinition>>;

const TWO: &[&str] = &["mono-green", "mono-red"];
const THREE: &[&str] = &["mono-red", "mono-white", "mono-green"];

fn requests(decks: &[&str]) -> Vec<SeatRequest> {
    decks
        .iter()
        .enumerate()
        .map(|(i, id)| SeatRequest {
            name: format!("Bot {i}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
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

/// A two-seat game with a log: the bots are seated, ready, and have given `answers` answers.
async fn started(
    registry: &Registry,
    seed: u64,
    answers: usize,
) -> (GameId, Vec<String>, Vec<Bot>) {
    let created = registry.create(&requests(TWO)).await.unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut bots = vec![
        Bot::join(&handle, 0, seed).await,
        Bot::join(&handle, 1, seed).await,
    ];
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &handle, answers).await, None);
    (created.id, tokens(&created), bots)
}

/// Replaces card `index` of the deck with an edited copy.
fn edit(cards: &mut Deck, index: usize, change: impl FnOnce(&mut CardDefinition)) {
    let mut card = (*cards[index]).clone();
    change(&mut card);
    cards[index] = Arc::new(card);
}

/// The fingerprint a deck had before a deploy made `change` to it.
fn fingerprint_before(deck: &str, change: impl FnOnce(&mut Deck)) -> DeckFingerprint {
    let mut cards = demo::find(deck).unwrap().cards();
    change(&mut cards);
    DeckFingerprint::of(&cards).unwrap()
}

/// A game to plant: what changed, the decks of its seats, the seat that changed and the
/// fingerprint that seat's deck had before the change.
type Case = (
    &'static str,
    &'static [&'static str],
    usize,
    DeckFingerprint,
);

/// A game whose seat `seat` had a deck that `change` has since made into the one on the server.
fn case(
    name: &'static str,
    decks: &'static [&'static str],
    seat: usize,
    change: impl FnOnce(&mut Deck),
) -> Case {
    (name, decks, seat, fingerprint_before(decks[seat], change))
}

/// Rewrites the fingerprint stored for `seat`, as if the deck had been changed by a deploy after
/// the game was created.
async fn store_fingerprint(pool: &PgPool, game: GameId, seat: usize, fingerprint: DeckFingerprint) {
    sqlx::query(
        "UPDATE games
         SET recipe = jsonb_set(recipe, ARRAY['fingerprints', $2::text], to_jsonb($3::text))
         WHERE id = $1",
    )
    .bind(game.uuid())
    .bind(seat.to_string())
    .bind(fingerprint.to_string())
    .execute(pool)
    .await
    .unwrap();
}

async fn stored_recipe(pool: &PgPool, game: GameId) -> Value {
    let record = GameRecords::new(pool.clone()).get(game).await.unwrap();
    record.unwrap().recipe
}

async fn event_count(pool: &PgPool, game: GameId) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM game_events WHERE game_id = $1")
        .bind(game.uuid())
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Collects everything the process logs, at every level.
#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Starts capturing the process's logs, once; tests of this file share the capture.
fn capture_logs() -> Arc<Mutex<Vec<u8>>> {
    static LOGS: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    LOGS.get_or_init(|| {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = Capture(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("one subscriber per process");
        buffer
    })
    .clone()
}

/// The log lines that mention `game`; games do not share ids, so tests do not see each other's.
fn log_lines_of(game: GameId) -> Vec<String> {
    let text = String::from_utf8_lossy(&capture_logs().lock().unwrap()).into_owned();
    let id = game.to_string();
    text.lines()
        .filter(|line| line.contains(&id))
        .map(str::to_owned)
        .collect()
}

#[sqlx::test(migrations = false)]
async fn a_game_whose_decks_are_unchanged_resumes_at_every_boot_with_its_log(pool: PgPool) {
    migrate(&pool).await.unwrap();
    capture_logs();
    let (seed, config) = (21, config(21));
    let registry = Registry::with_database(&config, pool.clone());
    let (running, tokens, _bots) = started(&registry, seed, 30).await;
    let lobby = registry
        .create(&requests(&["mono-white", "mono-white"]))
        .await
        .unwrap()
        .id;
    let before = registry.handle(running).unwrap().status().await.unwrap();
    assert!(before.started && before.events > 0);
    drop(registry);

    // Created now, the recipe holds the fingerprint of each seat's deck as the server builds it.
    let stored = stored_recipe(&pool, running).await;
    let expected: Vec<String> = TWO
        .iter()
        .map(|id| {
            let cards = demo::find(id).unwrap().cards();
            DeckFingerprint::of(&cards).unwrap().to_string()
        })
        .collect();
    assert_eq!(stored["fingerprints"], json!(expected));

    for boot in 1..=2 {
        let (registry, report) = reboot(&pool, &config).await;
        assert_eq!(
            report,
            ResumeReport {
                resumed: 2,
                lobbies: 1,
                ..ResumeReport::default()
            },
            "boot {boot}"
        );
        let after = registry.handle(running).unwrap().status().await.unwrap();
        assert_eq!(
            (after.events, after.state_hash),
            (before.events, before.state_hash)
        );
        for (seat, token) in tokens.iter().enumerate() {
            let access = registry.authenticate(running, token).unwrap();
            assert_eq!(access.seat.index(), seat);
        }
        assert!(registry.handle(lobby).is_some());
    }
    // Resuming reads the recipe and never rewrites it, and a checked game logs no warning.
    assert_eq!(stored_recipe(&pool, running).await, stored);
    for game in [running, lobby] {
        let lines = log_lines_of(game);
        let resumed = lines.iter().any(|line| line.contains("game resumed"));
        assert!(resumed, "{lines:?}");
        let unchecked = lines.iter().any(|l| l.contains("without a deck check"));
        assert!(!unchecked, "{lines:?}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_deck_that_changed_fails_its_game_with_the_reason_and_the_seat(pool: PgPool) {
    migrate(&pool).await.unwrap();
    capture_logs();
    let config = config(22);
    let registry = Registry::with_database(&config, pool.clone());

    let cases = vec![
        // A card that is not what it was.
        case("a different power", TWO, 0, |d| {
            edit(d, 30, |card| card.power = card.power.map(|power| power + 1));
        }),
        case("a different toughness", TWO, 1, |d| {
            edit(d, 40, |card| card.toughness = card.toughness.map(|t| t + 1));
        }),
        case("a different cost", TWO, 0, |d| {
            let cost = ManaCost::parse("{5}{G}").unwrap();
            edit(d, 50, |card| card.mana_cost = Some(cost));
        }),
        case("a different name", TWO, 1, |d| {
            edit(d, 26, |card| card.name.push('!'));
        }),
        case("a different type", TWO, 0, |d| {
            edit(d, 35, |card| {
                card.types.insert(CardType::Artifact);
            });
        }),
        case("a land that no longer taps for mana", TWO, 1, |d| {
            edit(d, 0, |card| card.abilities.clear());
        }),
        // The list is not what it was.
        case("two cards trading places", TWO, 0, |d| d.swap(23, 24)),
        case("the list reversed", TWO, 1, |d| d.reverse()),
        case("a card fewer", TWO, 0, |d| {
            d.pop();
        }),
        case("a copy more", TWO, 1, |d| d.push(d[0].clone())),
        case("a land swapped for a creature", TWO, 0, |d| {
            d[0] = d[24].clone();
        }),
        case("no cards", TWO, 1, Vec::clear),
        // The seat named is the one that changed, not the first one.
        case("the third of three seats", THREE, 2, |d| d.rotate_left(1)),
        case("the second of three seats", THREE, 1, |d| d.swap(0, 59)),
    ];

    let control = registry.create(&requests(TWO)).await.unwrap().id;
    let mut planted = Vec::new();
    for (name, decks, seat, old) in &cases {
        let created = registry.create(&requests(decks)).await.unwrap();
        store_fingerprint(&pool, created.id, *seat, *old).await;
        planted.push((*name, created.id, tokens(&created), *decks, *seat));
    }
    drop(registry);

    let (registry, report) = reboot(&pool, &config).await;
    let mut failed = report.failed.clone();
    failed.sort();
    let mut expected: Vec<GameId> = planted.iter().map(|(_, id, ..)| *id).collect();
    expected.sort();
    assert_eq!(failed, expected, "{report:?}");
    assert_eq!(
        (report.resumed, report.lobbies),
        (1, 1),
        "only the control game comes back"
    );
    assert!(registry.handle(control).is_some());

    let records = GameRecords::new(pool.clone());
    for (name, id, tokens, decks, seat) in &planted {
        let record = records.get(*id).await.unwrap().unwrap();
        assert_eq!(record.status, RecordStatus::Failed, "{name}");
        let reason = record.failure.unwrap_or_default();
        let lead = format!(
            "the deck changed: seat {seat} (demo deck {:?})",
            decks[*seat]
        );
        assert!(reason.starts_with(&lead), "{name}: {reason}");

        // Nobody can reach it as if it were running.
        assert!(registry.handle(*id).is_none(), "{name}");
        assert!(registry.describe(*id).await.is_none(), "{name}");
        for token in tokens {
            assert!(registry.authenticate(*id, token).is_none(), "{name}");
        }

        // The log names the game, the seat and the reason.
        let lines = log_lines_of(*id);
        let error = lines
            .iter()
            .find(|line| line.contains("ERROR") && line.contains("marked failed"))
            .unwrap_or_else(|| panic!("{name}: nothing logged in {lines:?}"));
        assert!(error.contains(&format!("seat={seat}")), "{name}: {error}");
        assert!(error.contains("the deck changed"), "{name}: {error}");
    }

    // A failed game is not tried again.
    let (_, second) = reboot(&pool, &config).await;
    assert_eq!((second.resumed, second.failed.len()), (1, 0));
}

#[sqlx::test(migrations = false)]
async fn a_running_game_is_refused_before_its_log_is_replayed_and_comes_back_once_the_deck_is_right(
    pool: PgPool,
) {
    migrate(&pool).await.unwrap();
    capture_logs();
    let (seed, config) = (23, config(23));
    let registry = Registry::with_database(&config, pool.clone());
    let (game, tokens, mut bots) = started(&registry, seed, 40).await;
    let stopped = registry.handle(game).unwrap().status().await.unwrap();
    drop(registry);
    let events = event_count(&pool, game).await;
    assert!(events > 10);

    // A deploy gave a creature of the second seat's deck another toughness.
    let recipe = stored_recipe(&pool, game).await;
    let right: DeckFingerprint = serde_json::from_value(recipe["fingerprints"][1].clone()).unwrap();
    let old = fingerprint_before("mono-red", |d| {
        edit(d, 30, |card| card.toughness = card.toughness.map(|t| t + 3));
    });
    assert_ne!(old, right);
    store_fingerprint(&pool, game, 1, old).await;

    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(report.failed, [game]);
    assert!(registry.is_empty());
    let record = GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, RecordStatus::Failed);
    let reason = record.failure.unwrap();
    assert!(
        reason.starts_with("the deck changed: seat 1 (demo deck \"mono-red\")"),
        "{reason}"
    );
    assert_eq!(
        event_count(&pool, game).await,
        events,
        "the log is left as it was"
    );
    drop(registry);

    // With the right fingerprint back and the status set by hand, as the docs say, the very same
    // game resumes where it stopped and plays on to its end.
    store_fingerprint(&pool, game, 1, right).await;
    sqlx::query("UPDATE games SET status = 'running', failure = NULL WHERE id = $1")
        .bind(game.uuid())
        .execute(&pool)
        .await
        .unwrap();
    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(report.resumed, 1);
    let handle = registry.handle(game).unwrap();
    let resumed = handle.status().await.unwrap();
    assert_eq!(
        (resumed.events, resumed.state_hash),
        (stopped.events, stopped.state_hash)
    );
    for (seat, token) in tokens.iter().enumerate() {
        let access = registry.authenticate(game, token).unwrap();
        bots[seat].rejoin(&access.handle).await;
    }
    assert!(drive(&mut bots, &handle, usize::MAX).await.is_some());
    let record = GameRecords::new(pool).get(game).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Finished);
}

#[sqlx::test(migrations = false)]
async fn a_recipe_stored_before_fingerprints_existed_resumes_unchecked_and_warns(pool: PgPool) {
    migrate(&pool).await.unwrap();
    capture_logs();
    let (seed, config) = (24, config(24));
    let registry = Registry::with_database(&config, pool.clone());
    let (legacy, tokens, mut bots) = started(&registry, seed, 30).await;
    let (checked, ..) = started(&registry, seed, 30).await;
    let lobby = registry.create(&requests(TWO)).await.unwrap().id;
    let before = registry.handle(legacy).unwrap().status().await.unwrap();
    drop(registry);

    // The state of the database before this feature: no `fingerprints` in two of the recipes.
    for game in [legacy, lobby] {
        sqlx::query("UPDATE games SET recipe = recipe - 'fingerprints' WHERE id = $1")
            .bind(game.uuid())
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            stored_recipe(&pool, game)
                .await
                .get("fingerprints")
                .is_none()
        );
    }

    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(
        report,
        ResumeReport {
            resumed: 3,
            lobbies: 1,
            ..ResumeReport::default()
        }
    );
    let after = registry.handle(legacy).unwrap().status().await.unwrap();
    assert_eq!(
        (after.events, after.state_hash),
        (before.events, before.state_hash)
    );

    for game in [legacy, lobby] {
        let lines = log_lines_of(game);
        let warning = lines
            .iter()
            .find(|line| line.contains("WARN"))
            .unwrap_or_else(|| panic!("no warning in {lines:?}"));
        assert!(warning.contains("without a deck check"), "{warning}");
    }
    let lines = log_lines_of(checked);
    assert!(lines.iter().all(|line| !line.contains("WARN")), "{lines:?}");

    // As before: the old tokens seat the bots again and the game goes on to its end.
    let handle = registry.handle(legacy).unwrap();
    for (seat, token) in tokens.iter().enumerate() {
        let access = registry.authenticate(legacy, token).unwrap();
        bots[seat].rejoin(&access.handle).await;
    }
    assert!(drive(&mut bots, &handle, usize::MAX).await.is_some());
    let record = GameRecords::new(pool).get(legacy).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Finished);
}

#[sqlx::test(migrations = false)]
async fn a_failed_game_is_gone_for_lookups_sockets_and_my_games(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let config = ServerConfig {
        fixed_seed: Some(25),
        ..bot_config()
    };
    let mut server = TestServer::start_on(pool.clone(), config.clone()).await;
    let ann = register(server.addr, "Ann", PASSWORD).await;
    let body = json!({ "format": "normal", "seats": [
        { "name": "Ann", "deck": "mono-green" },
        { "name": "Bo", "deck": "mono-red" },
    ]});
    let (game, tokens) = ann.create_game(server.addr, &body).await;
    let id: GameId = game.parse().unwrap();
    let (other, _) = ann.create_game(server.addr, &body).await;

    // Before the restart the game is there for everybody who asks.
    let lookup = format!("/api/games/{game}");
    assert_eq!(net::get(server.addr, &lookup).await.status, 200);
    let listed = listed_games(&ann, server.addr).await;
    assert!(listed.contains(&game) && listed.contains(&other));

    server.shutdown();
    server.finished().await.unwrap();
    drop(server);
    let old = fingerprint_before("mono-red", |d| {
        edit(d, 30, |card| card.power = card.power.map(|power| power + 1));
    });
    store_fingerprint(&pool, id, 1, old).await;

    let mut server = TestServer::start_on(pool.clone(), config).await;
    let addr = server.addr;
    assert!(server.registry.handle(id).is_none());
    let survivor = server.registry.handle(other.parse().unwrap());
    assert!(survivor.is_some(), "the other game came back");

    // The REST lookup and both socket routes answer as for a game that never existed.
    let never = net::get(addr, "/api/games/00000000-0000-4000-8000-000000000000").await;
    let gone = net::get(addr, &lookup).await;
    assert_eq!((gone.status, gone.text()), (404, never.text()));
    for token in &tokens {
        let (status, _) = refused_at(addr, &format!("/ws/{game}/{token}"), &[]).await;
        assert_eq!(status, 404);
    }
    let (status, _) = refused_at(addr, &format!("/ws/{game}"), &[("Cookie", &ann.cookie)]).await;
    assert_eq!(status, 404);

    // "My games" does not list a failed game, whatever the reason; the other one is still there.
    assert_eq!(listed_games(&ann, addr).await, [other]);

    let record = GameRecords::new(pool).get(id).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Failed);
    assert!(
        record
            .failure
            .unwrap()
            .starts_with("the deck changed: seat 1")
    );
    server.shutdown();
    server.finished().await.unwrap();
}

/// The ids of the games `GET /api/me/games` lists for the browser, newest first.
async fn listed_games(browser: &net::Browser, addr: std::net::SocketAddr) -> Vec<String> {
    let games = browser.games(addr).await;
    let id = |game: &Value| game["game"].as_str().unwrap().to_owned();
    games.iter().map(id).collect()
}
