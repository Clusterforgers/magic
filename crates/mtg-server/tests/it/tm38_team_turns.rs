//! Goal: with the default windows, a team table asks the right seats at the right time: in Two-Headed Giant both players of the active team are asked in their main phases and both draw, in Teams only the active player does; the lobby keeps the teams each seat chose and checks their sizes only at the start; whole 2v2 games of human and bot seats end in a team result.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::time::Duration;

use crate::common::{self, Client, windowed};
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::turn::Step;
use mtg_engine::{DecisionKind, Event, GameResult};
use mtg_protocol::events::{EventView, HiddenEvent};
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::GameId;
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

fn registry() -> Registry {
    Registry::new(&ServerConfig {
        game: windowed(),
        bot_delay: Duration::from_millis(50),
        ..ServerConfig::default()
    })
}

/// Seats side by side for Two-Headed Giant (0 0 1 1), alternating for Teams (0 1 0 1 ...).
fn teams(mode: TeamMode, seats: usize) -> Teams {
    let size = u32::try_from(seats / 2).unwrap();
    Teams::even(mode, 2, size)
}

fn options(teams: Teams, bots: &[usize]) -> GameOptions {
    GameOptions {
        house_rules: HouseRules {
            teams: Some(teams),
            ..HouseRules::default()
        },
        bots: bots
            .iter()
            .map(|seat| BotSeat {
                seat: PlayerId::from_index(*seat),
                level: BotLevel::Easy,
            })
            .collect(),
        ..GameOptions::default()
    }
}

/// What the human seats saw: the priority windows each was asked at (turn, active seat, step) and every draw of a
/// turn's draw step (turn, active seat, drawing seat).
#[derive(Default)]
struct Seen {
    asked: Vec<(u32, usize, usize, Step)>,
    draws: BTreeSet<(u32, usize, usize)>,
    result: Option<GameResult>,
}

/// Plays the human `seats` passively (they keep, pass and never attack or block) until turn `turns` begins or the
/// game ends, and notes what they saw.
async fn play(registry: &Registry, created: GameId, humans: &[usize], turns: u32) -> Seen {
    let handle = registry.handle(created).unwrap();
    let mut clients = Vec::new();
    for seat in humans {
        clients.push(Client::join(&handle, *seat).await);
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    let mut seen = Seen::default();
    let mut place: Vec<(u32, usize, Step)> = vec![(0, 0, Step::Untap); clients.len()];
    let mut draw_turn: Option<(u32, usize, bool)> = None;
    let outcome = tokio::time::timeout(Duration::from_secs(6 * 60 * 60), async {
        loop {
            let mut progressed = false;
            for (index, client) in clients.iter_mut().enumerate() {
                let Ok(message) = client.inbox.try_recv() else {
                    continue;
                };
                progressed = true;
                match message {
                    ServerMessage::Events(batch) => {
                        let turn = &batch.table.turn;
                        place[index] = (turn.number, turn.active.index(), turn.step);
                        if index != 0 {
                            continue;
                        }
                        for event in &batch.events {
                            match event {
                                EventView::Public(Event::TurnBegan { number, active, .. }) => {
                                    draw_turn = Some((*number, active.index(), false));
                                }
                                EventView::Public(Event::StepBegan { step }) => {
                                    if let Some(at) = draw_turn.as_mut() {
                                        at.2 = *step == Step::Draw;
                                    }
                                }
                                EventView::Public(Event::CardDrawn { player, .. })
                                | EventView::Hidden(HiddenEvent::CardDrawn { player }) => {
                                    if let Some((number, active, true)) = draw_turn {
                                        seen.draws.insert((number, active, player.index()));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    ServerMessage::Decision { id, decision, .. } => {
                        let (number, active, step) = place[index];
                        if number >= turns {
                            return None;
                        }
                        if matches!(decision.kind, DecisionKind::Priority(_)) {
                            seen.asked.push((number, active, client.seat.index(), step));
                        }
                        let answer = common::passive(&decision);
                        client
                            .say(
                                &handle,
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
            if !progressed {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    })
    .await
    .expect("the team game stalled");
    seen.result = outcome;
    seen
}

#[rstest]
#[case::giant_four(TeamMode::TwoHeadedGiant, 4)]
#[case::teams_four(TeamMode::Teams, 4)]
#[case::teams_six(TeamMode::Teams, 6)]
#[tokio::test(start_paused = true)]
async fn the_seats_of_the_active_team_are_asked_and_draw_as_the_mode_says(
    #[case] mode: TeamMode,
    #[case] seats: usize,
) {
    let registry = registry();
    let table = teams(mode, seats);
    let created = registry
        .create_with(&requests(seats), options(table.clone(), &[]), None)
        .await
        .unwrap();
    let humans: Vec<usize> = (0..seats).collect();
    let seen = play(&registry, created.id, &humans, 6).await;
    let team = |seat: usize| table.team_of(PlayerId::from_index(seat));
    let turn_of = |seat: usize, active: usize| {
        seat == active || (mode == TeamMode::TwoHeadedGiant && team(seat) == team(active))
    };
    // Every seat holds lands it never plays, so it can act in each main phase of its turns, and only there.
    let main_asks: BTreeSet<(u32, usize)> = seen
        .asked
        .iter()
        .filter(|(_, _, _, step)| step.is_main())
        .map(|(number, _, seat, _)| (*number, *seat))
        .collect();
    for (number, active, seat, step) in &seen.asked {
        if step.is_main() {
            assert!(
                turn_of(*seat, *active),
                "seat {seat} asked in a main phase of seat {active}'s turn {number}"
            );
        }
    }
    let turns: BTreeSet<(u32, usize)> = seen.asked.iter().map(|(n, a, _, _)| (*n, *a)).collect();
    for (number, active) in &turns {
        for seat in (0..seats).filter(|seat| turn_of(*seat, *active)) {
            assert!(
                main_asks.contains(&(*number, seat)),
                "seat {seat} not asked on turn {number}"
            );
        }
    }
    for (number, active, drawer) in &seen.draws {
        assert!(
            turn_of(*drawer, *active),
            "seat {drawer} drew on seat {active}'s turn {number}"
        );
    }
    let drawers: BTreeSet<(u32, usize)> = seen.draws.iter().map(|(n, _, d)| (*n, *d)).collect();
    for (number, active) in turns.iter().filter(|(number, _)| *number > 1) {
        for seat in (0..seats).filter(|seat| turn_of(*seat, *active)) {
            assert!(
                drawers.contains(&(*number, seat)),
                "seat {seat} did not draw on turn {number}"
            );
        }
    }
    assert!(turns.len() >= 4, "the game ran too few turns: {turns:?}");
}

#[rstest]
#[case::giant_humans_together(TeamMode::TwoHeadedGiant, &[0, 1], &[2, 3])]
#[case::teams_humans_apart(TeamMode::Teams, &[0, 1], &[2, 3])]
#[case::giant_human_and_bot(TeamMode::TwoHeadedGiant, &[0, 2], &[1, 3])]
#[tokio::test(start_paused = true)]
async fn a_two_v_two_of_humans_and_bots_ends_in_a_team_result(
    #[case] mode: TeamMode,
    #[case] humans: &[usize],
    #[case] bots: &[usize],
) {
    let registry = registry();
    let table = teams(mode, 4);
    let created = registry
        .create_with(&requests(4), options(table.clone(), bots), None)
        .await
        .unwrap();
    let seen = play(&registry, created.id, humans, u32::MAX).await;
    let Some(GameResult::Team(winner)) = seen.result else {
        panic!("no team result: {:?}", seen.result);
    };
    assert!(winner < 2);
    let status = registry.handle(created.id).unwrap().status().await.unwrap();
    assert_eq!(status.result, Some(GameResult::Team(winner)));
}

#[rstest]
#[case::giant(TeamMode::TwoHeadedGiant)]
#[case::teams(TeamMode::Teams)]
#[tokio::test(start_paused = true)]
async fn the_lobby_keeps_the_chosen_teams_and_checks_their_sizes_at_the_start(
    #[case] mode: TeamMode,
) {
    let registry = registry();
    let created = registry
        .create_with(&requests(4), options(teams(mode, 4), &[]), None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut clients = Vec::new();
    for seat in 0..4 {
        clients.push(Client::join(&handle, seat).await);
    }
    // Seat 3 joins team 0 on its own: three against one, kept exactly as chosen.
    let chosen = registry
        .set_seat_team(
            created.id,
            PlayerId::from_index(3),
            PlayerId::from_index(3),
            0,
        )
        .await
        .unwrap();
    let lopsided = match mode {
        TeamMode::TwoHeadedGiant => vec![0, 0, 1, 0],
        TeamMode::Teams => vec![0, 1, 0, 0],
    };
    assert_eq!(chosen.seats, lopsided);
    // A player moves only their own seat; the host moves any.
    let other = registry
        .set_seat_team(
            created.id,
            PlayerId::from_index(1),
            PlayerId::from_index(2),
            1,
        )
        .await;
    assert!(other.is_err());
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    let status = handle.status().await.unwrap();
    if mode == TeamMode::TwoHeadedGiant {
        assert!(
            !status.started,
            "Two-Headed Giant started three against one"
        );
        let fixed = registry
            .set_seat_team(created.id, HOST, PlayerId::from_index(1), 1)
            .await
            .unwrap();
        assert_eq!(fixed.seats, [0, 1, 1, 0]);
        let status = handle.status().await.unwrap();
        assert!(
            status.ready.is_empty(),
            "a change of teams takes every Ready back"
        );
        for client in &clients {
            client.say(&handle, ClientMessage::Ready).await;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    assert!(handle.status().await.unwrap().started);
}
