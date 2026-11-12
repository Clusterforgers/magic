//! Goal: a recorded game with bot seats stores them in its recipe, and a server that restarts on the same database hands those seats to bots again, so the game started before the restart plays on to its end with only the creator answering.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use crate::common::{self, Client, roomy};
use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::bots::{BotLevel, BotSeat};
use mtg_server::registry::{GameOptions, SeatRequest};
use mtg_server::{Registry, ServerConfig, demo, migrate};
use sqlx::PgPool;

fn config() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(11),
        game: roomy(),
        bot_delay: Duration::from_millis(20),
        ..ServerConfig::default()
    }
}

fn requests() -> Vec<SeatRequest> {
    ["mono-green", "mono-red", "mono-white"]
        .iter()
        .enumerate()
        .map(|(n, id)| SeatRequest {
            name: format!("Player {n}"),
            deck: demo::find(id).unwrap(),
        })
        .collect()
}

/// Answers seat 0's decisions passively until `stop` says so; the game's result, if it ended.
async fn play_seat_zero(
    handle: &mtg_server::GameHandle,
    me: &mut Client,
    mut stop: impl FnMut() -> bool,
) -> Option<mtg_engine::GameResult> {
    loop {
        if stop() {
            return None;
        }
        match me.inbox.recv().await? {
            ServerMessage::Decision { id, decision, .. } => {
                let answer = common::passive(&decision);
                me.say(
                    handle,
                    ClientMessage::Answer {
                        decision: id,
                        answer,
                    },
                )
                .await;
            }
            ServerMessage::GameOver { result } => return Some(result),
            _ => {}
        }
    }
}

#[sqlx::test(migrations = false)]
async fn bot_seats_are_played_again_after_a_restart(pool: PgPool) {
    migrate(&pool).await.unwrap();
    let config = config();
    let registry = Registry::with_database(&config, pool.clone());
    let options = GameOptions {
        bots: vec![
            BotSeat {
                seat: PlayerId::from_index(1),
                level: BotLevel::Normal,
            },
            BotSeat {
                seat: PlayerId::from_index(2),
                level: BotLevel::Easy,
            },
        ],
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests(), options, None)
        .await
        .unwrap();
    let recipe: serde_json::Value = sqlx::query_scalar("SELECT recipe FROM games WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&created.id.to_string()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        recipe["bots"],
        serde_json::json!([{"seat": 1, "level": "normal"}, {"seat": 2, "level": "easy"}])
    );
    let handle = registry.handle(created.id).unwrap();
    let mut me = Client::join(&handle, 0).await;
    me.say(&handle, ClientMessage::Ready).await;
    let mut answered = 0;
    let ended = play_seat_zero(&handle, &mut me, || {
        answered += 1;
        answered > 20
    })
    .await;
    assert_eq!(ended, None, "the game ended before the restart");
    drop(me);
    drop(handle);
    drop(registry);

    let rebooted = Registry::with_database(&config, pool.clone());
    rebooted.resume().await.unwrap();
    let handle = rebooted.handle(created.id).unwrap();
    let mut me = Client::join(&handle, 0).await;
    let result = tokio::time::timeout(
        Duration::from_secs(600),
        play_seat_zero(&handle, &mut me, || false),
    )
    .await
    .expect("the bots stalled the resumed game");
    assert!(result.is_some(), "the resumed game ended");
}
