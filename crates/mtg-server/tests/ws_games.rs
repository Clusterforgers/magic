//! Goal: bot players on real sockets finish whole games with 2 and 4 seats exactly as the same game played in process, a seat that drops (or is replaced) and returns with its token is resynced and the game still ends the same, and a token never carries another seat's messages or hidden cards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::format::Normal;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_protocol::ServerMessage;
use mtg_server::{ServerConfig, demo};
use mtg_testkit::{GameOutcome, fuzz_setup, play_game};
use net::bot::Stop;
use net::{TestServer, WsBot, bot_config};
use rstest::rstest;
use serde_json::{Value, json};

const DECKS: [&str; 4] = ["mono-green", "mono-red", "mono-white", "mono-green"];

/// The creation request for a table of `players` seats.
fn request(players: usize) -> Value {
    let seats: Vec<Value> = (0..players)
        .map(|i| json!({ "name": format!("Bot {i}"), "deck": DECKS[i] }))
        .collect();
    json!({ "format": "normal", "seats": seats })
}

/// The same game played straight on an engine with the same bots and seed.
fn straight(players: usize, seed: u64) -> GameOutcome {
    let decks = DECKS[..players]
        .iter()
        .map(|id| demo::find(id).unwrap().cards())
        .collect();
    let setup = GameSetup::new(Arc::new(Normal), seed, decks);
    play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"))
}

fn seeded(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        ..bot_config()
    }
}

/// Seats every bot before any is ready, so nobody needs a snapshot.
async fn seat_bots(server: &TestServer, game: &str, tokens: &[String], seed: u64) -> Vec<WsBot> {
    let mut bots = Vec::new();
    for (seat, token) in tokens.iter().enumerate() {
        bots.push(WsBot::connect(server.addr, game, token, seat, seed).await);
    }
    bots
}

/// Readies the bot and plays it to the end on its own task.
fn spawn_to_the_end(
    mut bot: WsBot,
) -> tokio::task::JoinHandle<(common::bot::BotReport, Vec<String>)> {
    tokio::spawn(async move {
        bot.ready().await;
        bot.finish_recorded().await
    })
}

#[rstest]
#[tokio::test]
async fn bots_on_sockets_play_the_game_the_engine_plays_alone(
    #[values(2, 4)] players: usize,
    #[values(1, 2)] seed: u64,
) {
    let alone = straight(players, seed);
    let server = TestServer::start(seeded(seed)).await;
    let (game, tokens) = net::create_game(server.addr, request(players)).await;
    let bots = seat_bots(&server, &game, &tokens, seed).await;
    let tasks: Vec<_> = bots.into_iter().map(spawn_to_the_end).collect();
    let mut reports = Vec::new();
    for task in tasks {
        reports.push(task.await.unwrap().0);
    }

    let id = game.parse().unwrap();
    let status = server.registry.handle(id).unwrap().status().await.unwrap();
    assert_eq!(status.events, alone.events.len() as u64);
    assert_eq!(status.state_hash, Some(alone.final_hash));
    assert_eq!(status.result, Some(alone.result));
    assert_eq!(status.waiting_on, None);

    let (mut answers, mut decisions) = (0, 0);
    for report in &reports {
        assert_eq!(report.result, alone.result);
        assert_eq!(report.events_seen, alone.events.len() as u64);
        assert_eq!(report.snapshots, 0, "nobody needed a resync");
        assert_eq!(report.errors, []);
        answers += report.answers;
        decisions += report.decisions;
    }
    assert_eq!(answers, alone.answers);
    assert_eq!(decisions, answers, "each decision reached exactly one seat");

    // The public view agrees.
    let reply = net::get(server.addr, &format!("/api/games/{game}")).await;
    assert_eq!(reply.status, 200);
    let view = reply.json();
    assert_eq!(view["started"], true);
    assert_eq!(view["result"], serde_json::to_value(alone.result).unwrap());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comeback {
    /// The old socket dies first and the server notices before the seat returns.
    HangUp,
    /// The seat opens a second socket while the first is still open.
    Replace,
}

#[rstest]
#[tokio::test]
async fn a_seat_that_returns_with_its_token_is_resynced_and_the_game_ends_the_same(
    #[values(2, 4)] players: usize,
    #[values(1, 7, 40)] pause_at: usize,
    #[values(Comeback::HangUp, Comeback::Replace)] comeback: Comeback,
) {
    let seed = 3;
    let seat0 = PlayerId::from_index(0);
    let alone = straight(players, seed);
    let server = TestServer::start(seeded(seed)).await;
    let (game, tokens) = net::create_game(server.addr, request(players)).await;
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    let mut bots = seat_bots(&server, &game, &tokens, seed).await;
    let mut victim = bots.remove(0);
    victim.pause_at = Some(pause_at);
    let others: Vec<_> = bots.into_iter().map(spawn_to_the_end).collect();
    victim.ready().await;

    // Seat 0 plays until it faces the n-th decision, then leaves with the answer unsent.
    assert_eq!(victim.play().await, Stop::Paused);
    let waiting = handle.status().await.unwrap().waiting_on;
    assert_eq!(waiting, Some(seat0));
    if comeback == Comeback::HangUp {
        victim.client.hang_up();
        for _ in 0..200 {
            if !handle.status().await.unwrap().connected.contains(&seat0) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let status = handle.status().await.unwrap();
        assert!(!status.connected.contains(&seat0), "the server noticed");
        assert_eq!(status.waiting_on, waiting, "the game waits for the seat");
    }

    let mut previous = victim.reconnect(server.addr, &game, &tokens[0]).await;
    if comeback == Comeback::Replace {
        assert_eq!(
            previous.closed().await,
            Some(4000),
            "the old socket is told"
        );
    }
    // The new socket gets Welcome, a snapshot and the pending decision, in that order.
    let first = [
        victim.client.message().await,
        victim.client.message().await,
        victim.client.message().await,
    ];
    assert!(matches!(&first[0], ServerMessage::Welcome { player, .. } if *player == seat0));
    assert!(
        matches!(&first[1], ServerMessage::Snapshot { state, .. } if state.table.viewer == seat0)
    );
    assert!(
        matches!(&first[2], ServerMessage::Decision { decision, .. } if decision.player == seat0)
    );
    for message in first {
        assert_eq!(victim.feed(message).await, None);
    }
    let report = victim.finish().await;
    assert_eq!(report.snapshots, 1);
    assert_eq!(report.result, alone.result);
    assert_eq!(report.events_seen, alone.events.len() as u64);
    for task in others {
        assert_eq!(task.await.unwrap().0.result, alone.result);
    }

    let status = handle.status().await.unwrap();
    assert_eq!(status.events, alone.events.len() as u64);
    assert_eq!(status.state_hash, Some(alone.final_hash));
    assert_eq!(status.result, Some(alone.result));
}

/// A fuzz setup whose every card has a distinct name that says whose it is.
fn unique_setup(players: usize, seed: u64) -> GameSetup {
    let mut setup = fuzz_setup(players, seed);
    for (seat, player) in setup.players.iter_mut().enumerate() {
        player.library = player
            .library
            .iter()
            .enumerate()
            .map(|(i, card)| {
                let mut def = (**card).clone();
                def.name = format!("Secret<p{seat}:c{i}>");
                Arc::new(def)
            })
            .collect();
    }
    setup.validate_decks = false;
    setup
}

/// The seat that owns a card called `Secret<p{seat}:c{i}>`.
fn owner(name: &str) -> Option<usize> {
    name.strip_prefix("Secret<p")?
        .split(':')
        .next()?
        .parse()
        .ok()
}

/// Every card name that appears in `texts`.
fn mentioned(texts: &[String]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for text in texts {
        let mut rest = text.as_str();
        while let Some(at) = rest.find("Secret<p") {
            rest = &rest[at..];
            let end = rest.find('>').map_or(rest.len(), |e| e + 1);
            found.insert(rest[..end].to_owned());
            rest = &rest[end..];
        }
    }
    found
}

#[rstest]
#[case(2, 5)]
#[case(4, 6)]
#[tokio::test]
async fn a_seats_sockets_never_carry_the_names_of_cards_hidden_from_it(
    #[case] players: usize,
    #[case] seed: u64,
) {
    let setup = unique_setup(players, seed);
    let alone = play_game(setup.clone(), seed).unwrap_or_else(|failure| panic!("{failure}"));
    let end = replay(&setup, &alone.events).unwrap();

    let server = TestServer::start(bot_config()).await;
    let names = (0..players).map(|i| format!("Bot {i}")).collect();
    let created = server.registry.register(setup, names).await.unwrap();
    let game = created.id.to_string();
    let tokens: Vec<String> = created
        .seats
        .iter()
        .map(|(_, token)| token.expose().to_owned())
        .collect();
    let bots = seat_bots(&server, &game, &tokens, seed).await;
    let tasks: Vec<_> = bots.into_iter().map(spawn_to_the_end).collect();
    let mut played = Vec::new();
    for task in tasks {
        played.push(task.await.unwrap());
    }

    for (report, texts) in &played {
        let me = report.seat.index();
        assert_eq!(report.result, alone.result);
        assert_eq!(report.errors, []);
        // A card that ends the game in a library or in someone else's hand never left hiding, so a
        // seat that does not own it must not have heard its name.
        let hidden: BTreeSet<String> = end
            .objects()
            .filter(|o| owner(&o.card().name).is_some_and(|seat| seat != me))
            .filter(|o| match o.zone() {
                Zone::Library(_) => true,
                Zone::Hand(holder) => holder.index() != me,
                _ => false,
            })
            .map(|o| o.card().name.clone())
            .collect();
        let heard = mentioned(texts);
        let leaked: Vec<_> = heard.intersection(&hidden).collect();
        assert!(leaked.is_empty(), "seat {me} was told {leaked:?}");
        // The scan is not vacuous: plenty was hidden, and the seat did hear of its own cards and of
        // cards on the table.
        assert!(hidden.len() > 20, "only {} hidden cards", hidden.len());
        assert!(heard.iter().any(|name| owner(name) == Some(me)));
        assert!(
            heard
                .iter()
                .any(|name| owner(name).is_some_and(|seat| seat != me))
        );
    }
}
