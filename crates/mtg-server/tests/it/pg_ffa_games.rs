//! Goal: a free-for-all Commander game at 2, 3, 4 and 6 seats runs from the lobby to its result over real sockets with two or three signed-in players and server bots in the other seats: everyone readies, takes an opening hand (the handicapped seat with its own life and hand size), plays turns with combat until it ends, and every seat and watcher is told the one result the record stores; a seat that is replaced and one that hangs up mid-game come back with their session and play on, a signed-in stranger who watches from the opening sees only public information, and the finished game replays to its seats and refuses guests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{net, pg_commander};

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::events::Event;
use mtg_engine::state::GameResult;
use mtg_engine::zones::Zone;
use mtg_protocol::{EventView, ServerMessage, TableView};
use mtg_server::{GameId, GameRecords, RecordStatus, ServerConfig};
use net::bot::Stop;
use net::{Browser, PASSWORD, TestServer, WsBot, WsClient, bot_config, open, register};
use pg_commander::{BEAST, create_body, join, led_by, on, store};
use serde_json::{Value, json};
use sqlx::PgPool;

const NAMES: [&str; 4] = ["ann", "bob", "cyd", "dee"];
const LEADERS: [&str; 3] = [
    "Questing Beast",
    "The Thing, Ben Grimm",
    "Rosheen Meanderer",
];

/// Short games: little life, a round limit, fast server bots and one seed.
fn config(seed: u64) -> ServerConfig {
    let mut config = bot_config();
    config.fixed_seed = Some(seed);
    config.bot_delay = Duration::from_millis(1);
    config.auth.limits.register_per_ip = 10_000;
    config.auth.limits.lobby_requests_per_ip = 10_000;
    config
}

/// Checks that a spectator's table lists no hand.
fn public_only(table: &TableView) {
    assert!(table.spectator);
    for player in &table.players {
        assert_eq!(player.hand.cards, None, "a hand listed to a spectator");
        assert_eq!(
            player.library.cards, None,
            "a library listed to a spectator"
        );
    }
}

/// What a watcher saw by the end of the game.
struct Watched {
    result: GameResult,
    attacks: usize,
    losses: usize,
}

/// Reads a spectator's socket to the end, checking every view it is sent is public.
async fn watch_to_the_end(mut socket: WsClient) -> Watched {
    let (mut attacks, mut losses) = (0, 0);
    loop {
        match socket.message().await {
            ServerMessage::Snapshot { state, .. } => public_only(&state.table),
            ServerMessage::Events(batch) => {
                public_only(&batch.table);
                for object in batch.objects.iter().chain(&batch.shown) {
                    assert!(
                        !matches!(object.zone, Zone::Hand(_) | Zone::Library(_)),
                        "a spectator was shown {} in a {:?}",
                        object.name,
                        object.zone
                    );
                }
                for event in &batch.events {
                    match event {
                        EventView::Public(Event::AttackersDeclared { .. }) => attacks += 1,
                        EventView::Public(Event::PlayerLost { .. }) => losses += 1,
                        _ => {}
                    }
                }
            }
            ServerMessage::Decision { .. } => panic!("a spectator was asked a decision"),
            ServerMessage::GameOver { result } => {
                socket.close().await;
                return Watched {
                    result,
                    attacks,
                    losses,
                };
            }
            _ => {}
        }
    }
}

/// Waits until the seat's socket is gone from the table.
async fn until_gone(server: &TestServer, game: GameId, seat: usize) {
    let handle = server.registry.handle(game).unwrap();
    for _ in 0..400 {
        if !handle
            .status()
            .await
            .unwrap()
            .connected
            .contains(&PlayerId::from_index(seat))
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the server never noticed seat {seat} hang up");
}

/// The stored record once the recorder has written the result.
async fn finished_record(pool: &PgPool, game: GameId) -> Option<GameResult> {
    let records = GameRecords::new(pool.clone());
    for _ in 0..200 {
        let record = records.get(game).await.unwrap().unwrap();
        if record.status == RecordStatus::Finished {
            return record.result;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the record never finished");
}

#[sqlx::test(migrations = false)]
async fn a_duel_of_two_players_runs_from_lobby_to_result(pool: PgPool) {
    free_for_all(pool, 2, 0, "normal").await;
}

#[sqlx::test(migrations = false)]
async fn three_seats_with_a_bot_run_from_lobby_to_result(pool: PgPool) {
    free_for_all(pool, 3, 1, "normal").await;
}

#[sqlx::test(migrations = false)]
async fn four_seats_with_a_hard_bot_run_from_lobby_to_result(pool: PgPool) {
    free_for_all(pool, 4, 1, "hard").await;
}

#[sqlx::test(migrations = false)]
async fn six_seats_with_three_bots_run_from_lobby_to_result(pool: PgPool) {
    free_for_all(pool, 6, 3, "easy").await;
}

/// Plays a Commander game of `seats` seats, `bots` of them server bots at `level`, to its end.
async fn free_for_all(pool: PgPool, seats: usize, bots: usize, level: &str) {
    let seed = 11;
    let server = pg_commander::start_with(&pool, config(seed)).await;
    let humans = seats - bots;
    let mut players: Vec<(Browser, String)> = Vec::new();
    for (index, name) in NAMES[..humans].iter().enumerate() {
        let browser = register(server.addr, name, PASSWORD).await;
        let deck = store(&server, &browser, &led_by(LEADERS[index % LEADERS.len()])).await;
        players.push((browser, deck));
    }
    let stranger = register(server.addr, "eve", PASSWORD).await;

    // The host holds seat 0 and the bot seats; the other players take the open seats behind them.
    let last = seats - 1;
    let mut seat_list = vec![json!({"name": "ann", "deck": players[0].1})];
    for _ in 0..bots {
        seat_list.push(json!({"deck": on(BEAST), "bot": true, "level": level}));
    }
    seat_list.resize(seats, json!({}));
    let mut body = create_body(&seat_list, 1 + bots);
    body["house_rules"] = json!({
        "starting_life": 8,
        "turn_limit": 10,
        "handicaps": [{ "seat": last, "starting_life": 12, "opening_hand_size": 6 }],
    });
    let (game, _) = players[0].0.create_game(server.addr, &body).await;
    let id: GameId = game.parse().unwrap();
    let mut seat_of = vec![0];
    for (who, deck) in &players[1..] {
        let joined = join(&server, who, &game, deck).await;
        assert_eq!(joined.status, 200, "{}", joined.text());
        seat_of.push(joined.json()["seat"].as_u64().unwrap() as usize);
    }
    assert_eq!(seat_of, [vec![0], (1 + bots..seats).collect()].concat());
    let watch_path = format!("/ws/{game}/watch");
    let mut early = open(
        server.addr,
        &watch_path,
        &[("Cookie", stranger.cookie.as_str())],
    )
    .await;
    early.join(&game).await;
    assert_eq!(early.closed().await, Some(1001), "nobody watches a lobby");

    let mut bots_on_sockets = Vec::new();
    for (index, (who, _)) in players.iter().enumerate() {
        bots_on_sockets.push(
            WsBot::connect_as_user(server.addr, &game, &who.cookie, seat_of[index], seed).await,
        );
    }
    // The last seat holds its first answer back, so the table waits on its opening hand.
    let mut victim = bots_on_sockets.pop().unwrap();
    assert_eq!(victim.seat, last);
    victim.pause_at = Some(1);
    let others: Vec<_> = bots_on_sockets
        .into_iter()
        .map(|mut bot| {
            tokio::spawn(async move {
                bot.ready().await;
                bot.finish().await
            })
        })
        .collect();
    victim.ready().await;
    assert_eq!(victim.play().await, Stop::Paused);

    // A stranger watches from the opening: the handicapped seat has its own life and hand size.
    let mut watching = open(
        server.addr,
        &watch_path,
        &[("Cookie", stranger.cookie.as_str())],
    )
    .await;
    assert_eq!(watching.welcome(&game).await, PlayerId::from_index(seats));
    let ServerMessage::Snapshot { state, .. } = watching.message().await else {
        panic!("a watcher starts with a snapshot");
    };
    public_only(&state.table);
    assert_eq!(state.table.format, "commander");
    for (seat, player) in state.table.players.iter().enumerate() {
        let life = if seat == last { 12 } else { 8 };
        assert_eq!(player.life, life, "seat {seat}");
        assert_eq!(player.commanders.len(), 1, "seat {seat}");
    }
    assert_eq!(state.table.players[last].hand.count, 6);
    let watcher = tokio::spawn(watch_to_the_end(watching));

    // The last seat opens a second socket (the first is closed), then plays on and hangs up mid-game.
    let cookie = players[humans - 1].0.cookie.clone();
    let mut replaced = victim.reconnect_as_user(server.addr, &game, &cookie).await;
    assert_eq!(replaced.closed().await, Some(4000));
    victim.pause_at = Some(victim.decisions + 25);
    assert_eq!(
        victim.play().await,
        Stop::Paused,
        "the game outlasts 25 more decisions"
    );
    victim.client.hang_up();
    until_gone(&server, id, last).await;
    let _ = victim.reconnect_as_user(server.addr, &game, &cookie).await;
    let report = victim.finish().await;
    assert_eq!(report.snapshots, 2, "each comeback is resynced");
    assert_eq!(report.errors, []);
    let result = report.result;
    for task in others {
        let report = task.await.unwrap();
        assert_eq!(report.result, result, "seat {}", report.seat.index());
        assert_eq!(report.errors, [], "seat {}", report.seat.index());
    }
    let watched = watcher.await.unwrap();
    assert_eq!(watched.result, result);
    assert!(watched.attacks > 0, "nobody attacked in ten rounds");
    if let GameResult::Winner(winner) = result {
        assert!(winner.index() < seats);
        assert!(watched.losses > 0 || seats == 2, "a winner without a loser");
    }

    // The record stores the result; the game replays to a seat and not to a guest.
    assert_eq!(finished_record(&pool, id).await, Some(result));
    let lookup: Value = net::get(server.addr, &format!("/api/games/{game}"))
        .await
        .json();
    assert_eq!(lookup["result"], serde_json::to_value(result).unwrap());
    let replay = players[1]
        .0
        .get(server.addr, &format!("/api/games/{game}/replay"))
        .await;
    assert_eq!(replay.status, 200, "{}", replay.text());
    let guest = net::get(server.addr, &format!("/api/games/{game}/replay")).await;
    assert_eq!(guest.status, 401);
}
