//! Goal: bot seats play whole games on their own: a table of 2 to 4 seats whose other seats are bots starts once the creator alone is ready, never stalls without any clock to push it, and ends in a result every seat is told; the recipe names the bot seats so a resumed game hands them to bots again; a change of teams, which takes every Ready back, leaves the bots ready.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use crate::common::{self, Client, roomy};
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::bots::{BotLevel, BotSeat};
use mtg_server::config::ServerConfig;
use mtg_server::demo;
use mtg_server::registry::{GameOptions, HOST, Registry, SeatRequest};
use rstest::rstest;

fn requests(seats: usize) -> Vec<SeatRequest> {
    (0..seats)
        .map(|n| SeatRequest {
            name: format!("Player {n}"),
            deck: &demo::all()[n % demo::all().len()],
        })
        .collect()
}

/// A registry whose games run no clock, so only the bots move their seats, and whose bots answer after 50 ms.
fn registry() -> Registry {
    Registry::new(&ServerConfig {
        game: roomy(),
        bot_delay: Duration::from_millis(50),
        ..ServerConfig::default()
    })
}

/// Bots at `level` for seats 1 to `seats - 1`.
fn bots(seats: usize, level: BotLevel) -> Vec<BotSeat> {
    (1..seats)
        .map(|seat| BotSeat {
            seat: PlayerId::from_index(seat),
            level,
        })
        .collect()
}

#[rstest]
#[case::easy_duel(2, BotLevel::Easy)]
#[case::easy_three(3, BotLevel::Easy)]
#[case::easy_four(4, BotLevel::Easy)]
#[case::normal_duel(2, BotLevel::Normal)]
#[case::hard_three(3, BotLevel::Hard)]
#[tokio::test(start_paused = true)]
async fn a_table_of_bots_plays_to_the_end(#[case] seats: usize, #[case] level: BotLevel) {
    let registry = registry();
    let options = GameOptions {
        bots: bots(seats, level),
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests(seats), options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut me = Client::join(&handle, 0).await;
    me.say(&handle, ClientMessage::Ready).await;
    // Seat 0 keeps its hand, passes and never blocks; the bots play the rest of the table.
    let result = tokio::time::timeout(Duration::from_secs(6 * 60 * 60), async {
        loop {
            match me.inbox.recv().await.expect("the table closed seat 0") {
                ServerMessage::Decision { id, decision, .. } => {
                    let answer = common::passive(&decision);
                    me.say(
                        &handle,
                        ClientMessage::Answer {
                            decision: id,
                            answer,
                        },
                    )
                    .await;
                }
                ServerMessage::GameOver { result } => return result,
                _ => {}
            }
        }
    })
    .await
    .expect("the bots stalled the game");
    let status = handle.status().await.unwrap();
    assert_eq!(status.result, Some(result));
    assert!(status.started);
}

#[tokio::test(start_paused = true)]
async fn the_bots_are_ready_and_the_game_waits_for_the_creator_alone() {
    let registry = registry();
    let options = GameOptions {
        bots: bots(3, BotLevel::Normal),
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests(3), options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let status = handle.status().await.unwrap();
    assert!(!status.started, "nobody started the game for the creator");
    let mut me = Client::join(&handle, 0).await;
    let roster = me
        .until(|message| match message {
            ServerMessage::Lobby { seats, .. } => Some(seats.clone()),
            _ => None,
        })
        .await;
    let ready: Vec<bool> = roster.iter().map(|seat| seat.ready).collect();
    let connected: Vec<bool> = roster.iter().map(|seat| seat.connected).collect();
    assert_eq!(ready, [false, true, true]);
    assert_eq!(connected, [true, true, true]);
    me.say(&handle, ClientMessage::Ready).await;
    me.decision().await;
    assert!(handle.status().await.unwrap().started);
}

#[rstest]
#[case::two_v_two(TeamMode::Teams)]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant)]
#[tokio::test(start_paused = true)]
async fn a_change_of_teams_leaves_the_bots_ready(#[case] mode: TeamMode) {
    let registry = registry();
    let options = GameOptions {
        bots: bots(4, BotLevel::Easy),
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests(4), options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut me = Client::join(&handle, 0).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let teams = Teams {
        mode,
        seats: vec![0, 1, 0, 1],
    };
    registry
        .set_teams(created.id, HOST, Some(teams))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    me.drain_all();
    me.say(&handle, ClientMessage::Ready).await;
    me.decision().await;
    assert!(handle.status().await.unwrap().started);
}
