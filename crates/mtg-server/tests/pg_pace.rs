//! Goal: a server that restarts on the same database brings back a game that plays with auto-pass and clocks on exactly as an uninterrupted one does under the same rule, no client is ever sent a decision it could do nothing with, and a seat's auto-pass setting is forgotten with the process (the server default applies again).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use common::{Bot, Client, drive, reference_auto_pass, roomy};
use mtg_engine::replay;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::registry::{Created, SeatRequest};
use mtg_server::{
    DeckRef, EventStore, GameConfig, GameRecipe, GameRecords, PgEventStore, RecordStatus, Registry,
    ResumeReport, ServerConfig, demo, migrate,
};
use sqlx::PgPool;

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

/// The server defaults for pace without the dwell (real time runs here): auto-pass on, 10 s to respond, 45 s to block.
fn config(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        game: GameConfig {
            pace: mtg_server::Pace {
                dwell: mtg_server::Dwell::OFF,
                ..mtg_server::Pace::default()
            },
            ..roomy()
        },
        ..ServerConfig::default()
    }
}

async fn reboot(pool: &PgPool, config: &ServerConfig) -> (Registry, ResumeReport) {
    let registry = Registry::with_database(config, pool.clone());
    let report = registry.resume().await.unwrap();
    (registry, report)
}

fn tokens(created: &Created) -> Vec<String> {
    created
        .seats
        .iter()
        .map(|(_, token)| token.expose().to_owned())
        .collect()
}

#[sqlx::test(migrations = false)]
async fn a_game_with_auto_pass_and_clocks_resumes_and_ends_like_an_uninterrupted_one(pool: PgPool) {
    migrate(&pool).await.unwrap();
    for (players, seed, answers_before) in [(2, 1, 4), (3, 2, 60), (4, 3, 200)] {
        let config = config(seed);
        let registry = Registry::with_database(&config, pool.clone());
        let created = registry.create(&requests(players)).await.unwrap();
        let tokens = tokens(&created);
        let handle = registry.handle(created.id).unwrap();
        let mut bots = Vec::new();
        for seat in 0..players {
            bots.push(Bot::join(&handle, seat, seed).await);
        }
        for bot in &bots {
            handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
        }
        assert_eq!(drive(&mut bots, &handle, answers_before).await, None);
        let stopped = handle.status().await.unwrap();
        drop(registry); // the process dies; the rows stay

        let (registry, report) = reboot(&pool, &config).await;
        assert_eq!(
            report,
            ResumeReport {
                resumed: 1,
                ..ResumeReport::default()
            }
        );
        let mut handle = None;
        for (seat, token) in tokens.iter().enumerate() {
            let access = registry.authenticate(created.id, token).unwrap();
            bots[seat].rejoin(&access.handle).await;
            handle = Some(access.handle);
        }
        let handle = handle.unwrap();
        let resumed = handle.status().await.unwrap();
        assert_eq!(resumed.state_hash, stopped.state_hash);
        assert_eq!(resumed.events, stopped.events);
        assert_eq!(resumed.waiting_on, stopped.waiting_on);

        let result = drive(&mut bots, &handle, usize::MAX).await;
        let alone = reference_auto_pass(recipe(players, seed).build().unwrap(), seed);
        assert_eq!(result, Some(alone.result), "{players} players");
        let logged = PgEventStore::new(pool.clone())
            .load(created.id)
            .await
            .unwrap();
        assert_eq!(logged, alone.events);
        let setup = recipe(players, seed).build().unwrap();
        assert_eq!(
            replay(&setup, &logged).unwrap().state_hash(),
            alone.final_hash
        );
        for bot in &bots {
            assert_eq!(bot.errors, []);
            assert_eq!(
                bot.idle_decisions, 0,
                "{:?} was asked for nothing",
                bot.seat
            );
        }
        let record = GameRecords::new(pool.clone())
            .get(created.id)
            .await
            .unwrap();
        assert_eq!(record.unwrap().status, RecordStatus::Finished);
    }
}

#[sqlx::test(migrations = false)]
async fn the_setting_is_forgotten_with_the_process(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let config = config(1);
    let registry = Registry::with_database(&config, pool.clone());
    let created = registry.create(&requests(2)).await.unwrap();
    let tokens = tokens(&created);
    let handle = registry.handle(created.id).unwrap();

    let mut seat = Client::join(&handle, 0).await;
    assert!(matches!(
        seat.next().await,
        ServerMessage::Welcome {
            auto_pass: true,
            ..
        }
    ));
    seat.say(&handle, ClientMessage::AutoPass { enabled: false })
        .await;
    let mut again = Client::join(&handle, 0).await;
    assert!(matches!(
        again.next().await,
        ServerMessage::Welcome {
            auto_pass: false,
            ..
        }
    ));
    drop((registry, handle));

    let (registry, report) = reboot(&pool, &config).await;
    assert_eq!(report.resumed, 1);
    let access = registry.authenticate(created.id, &tokens[0]).unwrap();
    let mut back = Client::join(&access.handle, 0).await;
    assert!(
        matches!(
            back.next().await,
            ServerMessage::Welcome {
                auto_pass: true,
                ..
            }
        ),
        "the setting is not stored: the server default applies again"
    );
}
