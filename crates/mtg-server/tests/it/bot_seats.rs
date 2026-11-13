//! Goal: bot seats play whole games on their own: seeded tables of 2 to 6 seats, teams too, at every level always finish within a bounded number of batches; a brain that hangs or panics is replaced and its seat plays on, and the clock answers for a bot that never answers in time; a table of 2 to 4 seats whose other seats are bots, free-for-all or in teams (2v2, Two-Headed Giant), starts once the creator alone is ready, never stalls without any clock to push it, and ends in a result every seat is told; the recipe names the bot seats so a resumed game hands them to bots again; a change of teams, which takes every Ready back, leaves the bots ready.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use crate::common::{self, Client, roomy};
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::Engine;
use mtg_engine::decision::{Answer, Decision, PriorityAction};
use mtg_engine::state::GameResult;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::Pace;
use mtg_server::bots::{BotBrain, BotLevel, BotSeat, spawn_bot_seat_with};
use mtg_server::config::ServerConfig;
use mtg_server::demo;
use mtg_server::registry::Removal;
use mtg_server::registry::{GameOptions, HOST, Registry, SeatRequest};
use mtg_testkit::brain::SeatBrain;
use rstest::rstest;
use tokio::sync::watch;

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
#[case::hard_duel(2, BotLevel::Hard)]
#[case::hard_three(3, BotLevel::Hard)]
#[case::normal_four(4, BotLevel::Normal)]
#[tokio::test(start_paused = true)]
async fn a_table_of_bots_plays_to_the_end(#[case] seats: usize, #[case] level: BotLevel) {
    play_against_bots(seats, level, None).await;
}

#[rstest]
#[case::two_v_two_easy(TeamMode::Teams, BotLevel::Easy)]
#[case::two_v_two_hard(TeamMode::Teams, BotLevel::Hard)]
#[case::giant_easy(TeamMode::TwoHeadedGiant, BotLevel::Easy)]
#[case::giant_normal(TeamMode::TwoHeadedGiant, BotLevel::Normal)]
#[tokio::test(start_paused = true)]
async fn a_team_table_of_bots_plays_to_a_team_result(
    #[case] mode: TeamMode,
    #[case] level: BotLevel,
) {
    let teams = Teams::even(mode, 2, 2);
    let result = play_against_bots(4, level, Some(teams)).await;
    assert!(
        matches!(result, GameResult::Team(0 | 1) | GameResult::Draw),
        "{result:?}"
    );
}

/// Seat 0 keeps its hand and passes while bots play the other seats; answers the result.
async fn play_against_bots(seats: usize, level: BotLevel, teams: Option<Teams>) -> GameResult {
    let registry = registry();
    let options = GameOptions {
        bots: bots(seats, level),
        house_rules: HouseRules {
            teams,
            ..HouseRules::default()
        },
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
    result
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

/// Plays a table of `seats` whose seats but the first are bots at `level`, under `teams` when given, from `seed`, with
/// seat 0 passive; returns how many event batches the game took. Panics when the game stalls.
async fn play_out(seats: usize, level: BotLevel, teams: Option<TeamMode>, seed: u64) -> usize {
    let registry = Registry::new(&ServerConfig {
        game: roomy(),
        bot_delay: Duration::from_millis(50),
        fixed_seed: Some(seed),
        ..ServerConfig::default()
    });
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
    if let Some(mode) = teams {
        let sides = (0..seats)
            .map(|seat| u32::try_from(seat % 2).unwrap())
            .collect();
        let teams = Teams { mode, seats: sides };
        registry
            .set_teams(created.id, HOST, Some(teams))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        me.drain_all();
    }
    me.say(&handle, ClientMessage::Ready).await;
    let mut batches = 0;
    tokio::time::timeout(Duration::from_secs(6 * 60 * 60), async {
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
                ServerMessage::Events(_) => {
                    batches += 1;
                    assert!(
                        batches < MAX_BATCHES,
                        "the game ran past {MAX_BATCHES} batches"
                    );
                }
                ServerMessage::GameOver { .. } => return,
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!("the bots stalled a game of {seats} seats at {level:?} from seed {seed}")
    });
    batches
}

/// More event batches than any game of demo decks takes: a game past it is looping.
const MAX_BATCHES: usize = 20_000;

#[rstest]
#[case::easy_two(2, BotLevel::Easy, None)]
#[case::normal_two(2, BotLevel::Normal, None)]
#[case::hard_two(2, BotLevel::Hard, None)]
#[case::easy_three(3, BotLevel::Easy, None)]
#[case::normal_four(4, BotLevel::Normal, None)]
#[case::hard_four(4, BotLevel::Hard, None)]
#[case::easy_five(5, BotLevel::Easy, None)]
#[case::easy_six(6, BotLevel::Easy, None)]
#[case::normal_six(6, BotLevel::Normal, None)]
#[case::normal_teams(4, BotLevel::Normal, Some(TeamMode::Teams))]
#[case::hard_two_headed_giant(4, BotLevel::Hard, Some(TeamMode::TwoHeadedGiant))]
#[case::easy_three_on_three(6, BotLevel::Easy, Some(TeamMode::Teams))]
#[tokio::test(start_paused = true)]
async fn seeded_tables_of_bots_always_finish(
    #[case] seats: usize,
    #[case] level: BotLevel,
    #[case] teams: Option<TeamMode>,
    #[values(11, 12, 13)] seed: u64,
) {
    play_out(seats, level, teams, seed).await;
}

/// A brain that fails every decision it is given: one never done in time, one that panics.
#[derive(Clone, Copy)]
enum Broken {
    Hangs(Duration),
    Panics,
}

impl SeatBrain for Broken {
    fn choose(&mut self, _decision: &Decision, _table: Option<&Engine>) -> Answer {
        match self {
            Broken::Hangs(long) => {
                std::thread::sleep(*long);
                Answer::Priority(PriorityAction::Pass)
            }
            Broken::Panics => panic!("a brain that panics"),
        }
    }
}

/// Plays a duel whose seat 1 is a bot that starts with `brain`, thinks up to `deadline` and plays under `pace`, until
/// the table reaches turn 3; panics when it does not within a minute.
async fn duel_reaches_turn_three(brain: Broken, deadline: Duration, pace: Option<Pace>) {
    let registry = Registry::new(&ServerConfig {
        game: roomy(),
        fixed_seed: Some(5),
        ..ServerConfig::default()
    });
    let options = GameOptions {
        pace,
        ..GameOptions::default()
    };
    let created = registry
        .create_with(&requests(2), options, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let (_alive, removed) = watch::channel(Removal::Dropped);
    let bot = BotSeat {
        seat: PlayerId(1),
        level: BotLevel::Easy,
    };
    let first = BotBrain::new(5, bot.seat, bot.level)
        .with_brain(Box::new(brain))
        .with_deadline(deadline);
    let _bot = spawn_bot_seat_with(
        handle.clone(),
        bot,
        5,
        first,
        Duration::from_millis(10),
        removed,
    );
    let mut me = Client::join(&handle, 0).await;
    me.say(&handle, ClientMessage::Ready).await;
    tokio::time::timeout(Duration::from_mins(1), async {
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
                ServerMessage::Events(batch) if batch.table.turn.number >= 3 => return,
                ServerMessage::GameOver { .. } => return,
                _ => {}
            }
        }
    })
    .await
    .expect("the broken bot stalled the game");
}

#[rstest]
#[case::hangs(Broken::Hangs(Duration::from_secs(2)))]
#[case::panics(Broken::Panics)]
#[tokio::test]
async fn a_brain_that_hangs_or_panics_is_replaced_and_the_bot_plays_on(#[case] brain: Broken) {
    duel_reaches_turn_three(brain, Duration::from_millis(100), None).await;
}

#[tokio::test]
async fn the_clock_answers_for_a_bot_that_never_answers_in_time() {
    let quick = Some(Duration::from_millis(200));
    let pace = Pace {
        response: quick,
        blocks: quick,
        think: quick,
        active: quick,
        extension: None,
        ..Pace::MANUAL
    };
    // The bot thinks far past the clock every time: only the table's fallback moves its seat on.
    duel_reaches_turn_three(
        Broken::Hangs(Duration::from_millis(400)),
        Duration::from_secs(60),
        Some(pace),
    )
    .await;
}
