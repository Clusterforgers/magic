//! Goal: a seat that has nothing to do is passed for at once, through the same path as its own answer (the log is the one a bare engine writes under the same rule), and its client is never sent that decision; the setting is per seat, switches mid-game, is reported in `Welcome`, resets on resume, and the run of passes stops the moment the game asks anything else.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    Client, FlakyStore, game_id, play_table, reference, reference_auto_pass, roomy, seat, silent,
    table_setup,
};
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{DecisionKind, PriorityAction, PriorityOptions};
use mtg_engine::state::GameResult;
use mtg_engine::{Answer, Decision, Event, GameSetup, replay};
use mtg_protocol::{ClientMessage, DecisionId, ErrorCode, ServerMessage};
use mtg_server::{
    EventStore, GameConfig, GameHandle, MemoryStore, Pace, resume_game_with, spawn_game_with,
};
use mtg_testkit::cards::{bear, forest, free_creature, giant};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

/// Auto-pass on.
const ON: Pace = Pace { auto_pass: true };

fn config(pace: Pace) -> GameConfig {
    GameConfig { pace, ..roomy() }
}

/// Two players, nothing in either hand: both are idle until a land turns up in a hand on turn 2.
fn quiet() -> GameSetup {
    Scenario::new(2).setup().unwrap()
}

fn spawn(setup: GameSetup, pace: Pace) -> (GameHandle, MemoryStore) {
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config(pace)).unwrap();
    (handle, store)
}

/// Seats `seats` clients, readies them all and returns them once the actor has dealt with the starts.
async fn join_and_ready(handle: &GameHandle, seats: usize) -> Vec<Client> {
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(handle, n).await);
    }
    for client in &clients {
        client.say(handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    clients
}

fn decisions(messages: &[ServerMessage]) -> Vec<(DecisionId, Decision)> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Decision { id, decision, .. } => Some((*id, decision.clone())),
            _ => None,
        })
        .collect()
}

fn batches(messages: &[ServerMessage]) -> usize {
    messages
        .iter()
        .filter(|m| matches!(m, ServerMessage::Events(_)))
        .count()
}

fn priority(decision: &Decision) -> &PriorityOptions {
    match &decision.kind {
        DecisionKind::Priority(options) => options,
        other => panic!("expected a priority decision, got {other:?}"),
    }
}

/// The one decision among `messages`.
fn only_decision(messages: &[ServerMessage]) -> (DecisionId, Decision) {
    let mut found = decisions(messages);
    assert_eq!(found.len(), 1, "expected one decision in {messages:?}");
    found.remove(0)
}

async fn answer(handle: &GameHandle, from: &Client, id: DecisionId, answer: Answer) {
    from.say(
        handle,
        ClientMessage::Answer {
            decision: id,
            answer,
        },
    )
    .await;
    handle.status().await.unwrap();
}

async fn set(handle: &GameHandle, from: &Client, enabled: bool) {
    from.say(handle, ClientMessage::AutoPass { enabled }).await;
    handle.status().await.unwrap();
}

/// Plays a bot game with the server defaults (auto-pass on) and checks it against the bare engine.
async fn bot_game_matches_the_bare_engine_under_the_same_rule(players: usize, seed: u64) {
    let (handle, store) = spawn(table_setup(players, seed), Pace::default());
    let reports = play_table(&handle, players, seed).await;

    let straight = reference_auto_pass(table_setup(players, seed), seed);
    assert!(straight.auto_passes > 0, "the table has idle windows");
    assert_eq!(store.load(game_id(1)).await.unwrap(), straight.events);
    assert_eq!(
        handle.status().await.unwrap().state_hash,
        Some(straight.final_hash)
    );
    let asked: usize = reports.iter().map(|r| r.answers).sum();
    assert_eq!(asked, straight.answers - straight.auto_passes);
    for report in &reports {
        assert_eq!(report.result, straight.result);
        assert_eq!(
            report.idle_decisions, 0,
            "{:?} was asked for nothing",
            report.seat
        );
        assert_eq!(report.errors, []);
    }
}

#[rstest]
#[tokio::test]
async fn bot_games_match_the_bare_engine_under_the_same_rule_and_no_client_is_asked_for_nothing(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3)] seed: u64,
) {
    bot_game_matches_the_bare_engine_under_the_same_rule(players, seed).await;
}

#[rstest]
#[tokio::test]
async fn with_the_setting_off_idle_priority_still_reaches_its_seat(
    #[values(2, 3)] players: usize,
    #[values(1, 2)] seed: u64,
) {
    let (handle, store) = spawn(table_setup(players, seed), Pace::MANUAL);
    let reports = play_table(&handle, players, seed).await;

    let straight = reference(players, seed);
    assert_eq!(store.load(game_id(1)).await.unwrap(), straight.events);
    let idle: usize = reports.iter().map(|r| r.idle_decisions).sum();
    assert!(idle > 0, "every priority window is delivered");
    assert_eq!(
        reports.iter().map(|r| r.answers).sum::<usize>(),
        straight.answers
    );
}

#[tokio::test]
async fn a_seat_with_nothing_to_do_is_passed_for_and_never_sent_the_decision() {
    let (handle, store) = spawn(quiet(), ON);
    let mut clients = join_and_ready(&handle, 2).await;

    let first = clients[0].drain();
    let second = clients[1].drain();
    assert_eq!(decisions(&first), [], "seat 0 had nothing to do all turn");
    assert!(batches(&first) >= 1 && batches(&second) >= 1);
    let (_, decision) = only_decision(&second);
    let options = priority(&decision);
    assert!(options.can_act && options.playable_lands.len() == 1);

    let status = handle.status().await.unwrap();
    assert_eq!(status.waiting_on, Some(seat(1)));
    let logged = store.load(game_id(1)).await.unwrap();
    assert_eq!(status.events, logged.len() as u64);
    assert_eq!(
        replay(&quiet(), &logged).unwrap().state_hash(),
        status.state_hash.unwrap()
    );
    let passes = logged
        .iter()
        .filter(|e| matches!(e, Event::PriorityPassed { .. }))
        .count();
    assert!(passes >= 8, "the log shows the passes: {passes}");
}

#[tokio::test]
async fn a_seat_can_switch_auto_pass_off_and_on_in_the_middle_of_a_game() {
    let (handle, store) = spawn(quiet(), ON);
    let mut clients = join_and_ready(&handle, 2).await;
    let (land_window, decision) = only_decision(&clients[1].drain());
    let land = priority(&decision).playable_lands[0];
    clients[0].drain();

    // Off, nothing happens by itself.
    set(&handle, &clients[1], false).await;
    silent(&clients[1].drain());

    // The land is played; seat 1 now has nothing to do, and with the setting off it is asked all the same.
    let play = Answer::Priority(PriorityAction::PlayLand(land));
    answer(&handle, &clients[1], land_window, play).await;
    let received = clients[1].drain();
    let (idle_window, idle) = only_decision(&received);
    assert!(!priority(&idle).can_act && !priority(&idle).mana_abilities.is_empty());
    assert_eq!(batches(&received), 1);
    assert_eq!(decisions(&clients[0].drain()), []);

    // On again: the pending window is passed at once and the table moves on to seat 0's next land.
    set(&handle, &clients[1], true).await;
    let received = clients[1].drain();
    assert_eq!(
        batches(&received),
        1,
        "the pass reaches the seat that asked"
    );
    assert_eq!(decisions(&received), []);
    let (turn_three, decision) = only_decision(&clients[0].drain());
    assert_eq!(priority(&decision).playable_lands.len(), 1);
    assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(0)));

    // The window that was passed for seat 1 is gone.
    answer(
        &handle,
        &clients[1],
        idle_window,
        Answer::Priority(PriorityAction::Pass),
    )
    .await;
    assert!(matches!(
        clients[1].drain().as_slice(),
        [ServerMessage::Error {
            code: ErrorCode::StaleDecision,
            ..
        }]
    ));

    // Seat 1 asks to be stopped again; seat 0 is still passed for, so seat 1 is asked in seat 0's turn.
    set(&handle, &clients[1], false).await;
    let land = priority(&decision).playable_lands[0];
    answer(
        &handle,
        &clients[0],
        turn_three,
        Answer::Priority(PriorityAction::PlayLand(land)),
    )
    .await;
    let (_, asked) = only_decision(&clients[1].drain());
    assert!(!priority(&asked).can_act);
    assert_eq!(decisions(&clients[0].drain()), []);
    let status = handle.status().await.unwrap();
    assert_eq!(status.waiting_on, Some(seat(1)));

    let logged = store.load(game_id(1)).await.unwrap();
    assert_eq!(
        replay(&quiet(), &logged).unwrap().state_hash(),
        status.state_hash.unwrap()
    );
}

#[tokio::test]
async fn welcome_reports_the_setting_and_it_lives_through_a_reconnect() {
    let (handle, _) = spawn(quiet(), ON);
    let mut first = Client::join(&handle, 0).await;
    assert!(matches!(
        first.next().await,
        ServerMessage::Welcome {
            auto_pass: true,
            ..
        }
    ));
    set(&handle, &first, false).await;

    let mut again = Client::join(&handle, 0).await;
    assert!(matches!(
        again.next().await,
        ServerMessage::Welcome {
            auto_pass: false,
            ..
        }
    ));
    let mut other = Client::join(&handle, 1).await;
    assert!(
        matches!(
            other.next().await,
            ServerMessage::Welcome {
                auto_pass: true,
                ..
            }
        ),
        "another seat keeps its own"
    );
}

#[tokio::test]
async fn a_setting_sent_in_the_lobby_holds_when_the_game_starts() {
    let (handle, _) = spawn(quiet(), ON);
    let mut clients = vec![
        Client::join(&handle, 0).await,
        Client::join(&handle, 1).await,
    ];
    set(&handle, &clients[1], false).await;
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();

    assert_eq!(decisions(&clients[0].drain()), []);
    let (_, decision) = only_decision(&clients[1].drain());
    assert!(
        !priority(&decision).can_act,
        "seat 1 asked to be stopped even at an idle window"
    );
    assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(1)));
}

#[rstest]
#[case::an_attack_to_declare(
    Scenario::new(2).battlefield(0, [ready(free_creature("Raider", 2, 2))]),
    "declare-attackers"
)]
#[case::a_discard_at_the_end_of_the_turn(Scenario::new(2).hand(0, vec![giant(); 9]), "discard")]
#[case::a_land_to_play_at_the_start(Scenario::new(2).hand(0, [forest()]), "priority")]
#[case::a_spell_to_cast_at_the_start(
    Scenario::new(2).hand(0, [bear()]).battlefield(0, [ready(forest()), ready(forest())]),
    "priority"
)]
#[tokio::test]
async fn the_run_of_passes_stops_the_moment_the_game_asks_anything_else(
    #[case] scenario: Scenario,
    #[case] asked: &str,
) {
    let (handle, _) = spawn(scenario.setup().unwrap(), ON);
    let mut clients = join_and_ready(&handle, 2).await;

    let (_, decision) = only_decision(&clients[0].drain());
    assert_eq!(decision.kind.name(), asked);
    assert_eq!(decision.player, PlayerId(0));
    assert_eq!(decisions(&clients[1].drain()), []);
    assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(0)));
    if asked == "priority" {
        assert!(priority(&decision).can_act);
    }
}

#[tokio::test]
async fn a_table_where_nobody_can_act_plays_to_the_end() {
    // Seat 1 has no library: it loses on its first draw, after everything before it was passed.
    let setup = Scenario::new(2).library(1, []).setup().unwrap();
    let (handle, _) = spawn(setup, ON);
    let mut clients = join_and_ready(&handle, 2).await;

    for client in &mut clients {
        let messages = client.drain();
        assert_eq!(decisions(&messages), []);
        assert!(matches!(
            messages.last(),
            Some(ServerMessage::GameOver {
                result: GameResult::Winner(PlayerId(0))
            })
        ));
    }
    let status = handle.status().await.unwrap();
    assert_eq!(status.result, Some(GameResult::Winner(PlayerId(0))));
    assert_eq!(status.waiting_on, None);
}

#[tokio::test]
async fn a_failed_save_takes_back_the_answer_and_the_passes_that_followed_it() {
    let store = FlakyStore::new();
    let handle = spawn_game_with(game_id(1), quiet(), store.clone(), config(ON)).unwrap();
    let mut clients = join_and_ready(&handle, 2).await;
    let (window, decision) = only_decision(&clients[1].drain());
    clients[0].drain();
    let land = priority(&decision).playable_lands[0];
    let before = handle.status().await.unwrap();
    let saved = store.inner().load(game_id(1)).await.unwrap();
    let play = Answer::Priority(PriorityAction::PlayLand(land));

    store.fail(true);
    answer(&handle, &clients[1], window, play.clone()).await;
    assert!(matches!(
        clients[1].drain().as_slice(),
        [ServerMessage::Error {
            code: ErrorCode::Internal,
            ..
        }]
    ));
    silent(&clients[0].drain());
    assert_eq!(
        handle.status().await.unwrap(),
        before,
        "nothing of the move or the passes after it happened"
    );
    assert_eq!(store.inner().load(game_id(1)).await.unwrap(), saved);

    store.fail(false);
    answer(&handle, &clients[1], window, play).await;
    let after = handle.status().await.unwrap();
    assert!(after.events > before.events + 1, "the land and the passes");
    assert_eq!(after.waiting_on, Some(seat(0)));
    let logged = store.inner().load(game_id(1)).await.unwrap();
    assert_eq!(
        replay(&quiet(), &logged).unwrap().state_hash(),
        after.state_hash.unwrap()
    );
}

#[tokio::test]
async fn a_resumed_game_passes_for_an_idle_seat_before_anyone_connects() {
    let store = MemoryStore::new();
    let setup = quiet();
    // First life: nobody is passed for, so the game waits at seat 0's first idle window.
    let first = spawn_game_with(
        game_id(1),
        setup.clone(),
        store.clone(),
        config(Pace::MANUAL),
    )
    .unwrap();
    let clients = join_and_ready(&first, 2).await;
    let stopped = first.status().await.unwrap();
    assert_eq!(stopped.waiting_on, Some(seat(0)));
    let stored = store.load(game_id(1)).await.unwrap();
    drop(clients);
    drop(first);

    let second = resume_game_with(store.clone(), game_id(1), setup.clone(), config(ON))
        .await
        .unwrap();
    let status = second.status().await.unwrap();
    assert!(status.events > stopped.events);
    assert_eq!(status.waiting_on, Some(seat(1)), "seat 1's first land drop");
    assert_eq!(status.connected, Vec::<PlayerId>::new());
    let logged = store.load(game_id(1)).await.unwrap();
    assert_eq!(
        logged[..stored.len()],
        stored[..],
        "the old log is untouched"
    );
    assert_eq!(logged.len() as u64, status.events);
    assert_eq!(
        replay(&setup, &logged).unwrap().state_hash(),
        status.state_hash.unwrap()
    );

    let mut back = Client::join(&second, 1).await;
    assert!(matches!(
        back.next().await,
        ServerMessage::Welcome {
            auto_pass: true,
            ..
        }
    ));
    assert!(matches!(back.next().await, ServerMessage::Snapshot { .. }));
    let (_, decision) = back.decision().await;
    assert_eq!(priority(&decision).playable_lands.len(), 1);
}

#[tokio::test]
async fn the_setting_is_forgotten_when_the_game_is_resumed() {
    let store = MemoryStore::new();
    let setup = quiet();
    let first = spawn_game_with(game_id(1), setup.clone(), store.clone(), config(ON)).unwrap();
    let clients = join_and_ready(&first, 2).await;
    set(&first, &clients[1], false).await;
    let mut rejoined = Client::join(&first, 1).await;
    assert!(matches!(
        rejoined.next().await,
        ServerMessage::Welcome {
            auto_pass: false,
            ..
        }
    ));
    drop(clients);
    drop(rejoined);
    drop(first);

    let second = resume_game_with(store, game_id(1), setup, config(ON))
        .await
        .unwrap();
    let mut back = Client::join(&second, 1).await;
    assert!(matches!(
        back.next().await,
        ServerMessage::Welcome {
            auto_pass: true,
            ..
        }
    ));
}

#[tokio::test]
async fn a_very_long_run_of_idle_passes_is_cut_and_goes_on_with_the_next_answer() {
    // Four players whose libraries hold nothing they can play: dozens of turns of nothing to do, until a
    // hand outgrows its limit. One answer never has to carry all of it.
    let mut scenario = Scenario::new(4);
    for seat in 0..4 {
        scenario = scenario.library(seat, vec![giant(); 30]);
    }
    let setup = scenario.setup().unwrap();
    let (handle, store) = spawn(setup.clone(), ON);
    let mut clients = Vec::new();
    for n in 0..4 {
        clients.push(Client::join(&handle, n).await);
    }
    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }

    let mut idle_windows = 0;
    let discard = loop {
        let waiting = handle.status().await.unwrap().waiting_on.unwrap().index();
        let (id, decision) = clients[waiting].decision().await;
        match &decision.kind {
            DecisionKind::Priority(options) => {
                assert!(!options.can_act, "nothing here can be played");
                idle_windows += 1;
                assert!(idle_windows < 10, "the run is cut in pieces, not worn down");
                let pass = Answer::Priority(PriorityAction::Pass);
                answer(&handle, &clients[waiting], id, pass).await;
            }
            DecisionKind::Discard(_) => break decision,
            other => panic!("unexpected {other:?}"),
        }
    };
    assert!(
        idle_windows >= 1,
        "the first run was cut, so a priority window reached its seat"
    );
    assert_eq!(
        discard.player,
        PlayerId(1),
        "seat 1 is first over the limit: the scenario starts after seat 0's draw"
    );
    let logged = store.load(game_id(1)).await.unwrap();
    assert_eq!(
        replay(&setup, &logged).unwrap().state_hash(),
        handle.status().await.unwrap().state_hash.unwrap()
    );
}
