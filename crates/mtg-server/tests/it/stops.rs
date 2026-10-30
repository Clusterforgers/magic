//! Goal: a seat's stops and Full Control are held per seat from the lobby on and reported back: a change is told to that seat alone, as the whole list or the new mode, after the events of the step that caused it and before the decision that follows; a request that changes nothing says nothing and one the rules refuse (a step where nobody gets priority, a seat that is out) is a small bad-message error that changes nothing and needs no resync; a reconnect is told what the server holds and a resumed game starts with none; a seat that left, a finished game and a failed save leave them as the rules say; and play is exactly the same with them set.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{
    Bot, Client, FlakyStore, game_id, passive, poke, roomy, seat, silent, table_setup, windowed,
};
use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_engine::{GameSetup, replay};
use mtg_protocol::{ClientMessage, ErrorCode, FullControlMode, ServerMessage, Stop, StopSide};
use mtg_server::{EventStore, GameHandle, MemoryStore, resume_game_with, spawn_game_with};
use mtg_testkit::Scenario;
use mtg_testkit::cards::instant;
use rstest::rstest;

const SIDES: [StopSide; 2] = [StopSide::Own, StopSide::Opponent];

const FOE: usize = 0;
const ME: usize = 1;

/// Seat 0 (the active player, in its first main phase) and seat 1 each hold a free instant, so both can act.
fn instant_duel() -> GameSetup {
    Scenario::new(2)
        .hand(0, [instant("Foe Zap", 0, &[])])
        .hand(1, [instant("My Zap", 0, &[])])
        .setup()
        .unwrap()
}

/// A table of `seats` that has joined; with `started` it has also said it is ready. Queues are left unread.
async fn table(
    setup: GameSetup,
    seats: usize,
    started: bool,
) -> (GameHandle, FlakyStore, Vec<Client>) {
    let store = FlakyStore::new();
    let handle = spawn_game_with(game_id(1), setup, store.clone(), windowed()).unwrap();
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(&handle, n).await);
    }
    if started {
        for client in &clients {
            client.say(&handle, ClientMessage::Ready).await;
        }
    }
    handle.status().await.unwrap();
    (handle, store, clients)
}

fn stop(side: StopSide, step: Step, keep: bool) -> Stop {
    Stop { side, step, keep }
}

/// The request that sets `stop` with the `keep` it carries, or with `on` false clears the stop at its side and step.
fn set(stop: Stop, on: bool) -> ClientMessage {
    ClientMessage::SetStop {
        side: stop.side,
        step: stop.step,
        on,
        keep: stop.keep,
    }
}

fn full_control(mode: FullControlMode) -> ClientMessage {
    ClientMessage::FullControl { mode }
}

/// The lists of stops the seat was told, in order.
fn stops_told(messages: &[ServerMessage]) -> Vec<Vec<Stop>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Stops { stops } => Some(stops.clone()),
            _ => None,
        })
        .collect()
}

/// The Full Control modes the seat was told, in order.
fn modes_told(messages: &[ServerMessage]) -> Vec<FullControlMode> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::FullControl { mode } => Some(*mode),
            _ => None,
        })
        .collect()
}

fn is_error(messages: &[ServerMessage], expected: ErrorCode) -> bool {
    matches!(messages, [ServerMessage::Error { code, .. }] if *code == expected)
}

/// Connects the seat again, as a reconnect does, and returns what the `Welcome` of the new connection states.
async fn reconnect(handle: &GameHandle, client: &mut Client) -> (Vec<Stop>, FullControlMode) {
    *client = Client::join(handle, client.seat.index()).await;
    match client.next().await {
        ServerMessage::Welcome {
            stops,
            full_control,
            ..
        } => (stops, full_control),
        other => panic!("expected a welcome, got {other:?}"),
    }
}

/// The kinds of message in the order received, a run of event batches counted once.
fn order(messages: &[ServerMessage]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for message in messages {
        let now = match message {
            ServerMessage::Events(_) => "events",
            ServerMessage::Decision { .. } => "decision",
            ServerMessage::Stops { .. } => "stops",
            ServerMessage::FullControl { .. } => "full control",
            ServerMessage::Yielding { .. } => "yielding",
            ServerMessage::GameOver { .. } => "game over",
            ServerMessage::Error { .. } => "error",
            ServerMessage::Snapshot { .. } => "snapshot",
            ServerMessage::Welcome { .. } => "welcome",
            ServerMessage::Chat { .. } => "chat",
            ServerMessage::Forecast { .. } => "forecast",
            ServerMessage::Lobby { .. } => "lobby",
            ServerMessage::Bank { .. } => "bank",
        };
        if now != "events" || out.last().is_none_or(|last| *last != "events") {
            out.push(now);
        }
    }
    out
}

#[rstest]
#[tokio::test]
async fn every_step_on_both_sides_is_set_and_cleared_or_refused_by_the_rule(
    #[values(false, true)] started: bool,
) {
    let (handle, _, mut clients) = table(instant_duel(), 2, started).await;
    for side in SIDES {
        for step in Step::ALL {
            // Both kinds of stop follow the same rule.
            for keep in [false, true] {
                let target = stop(side, step, keep);
                let told = poke(&handle, &mut clients, ME, set(target, true)).await;
                silent(&told[FOE]);
                if step.grants_priority() {
                    let said = format!("{side:?} {step:?} {keep}");
                    assert_eq!(stops_told(&told[ME]), [vec![target]], "{said}");
                    assert_eq!(told[ME].len(), 1, "and nothing else: {:?}", told[ME]);
                    let told = poke(&handle, &mut clients, ME, set(target, false)).await;
                    assert_eq!(stops_told(&told[ME]), [Vec::<Stop>::new()], "{said}");
                    silent(&told[FOE]);
                } else {
                    // Not a stale or illegal answer, so the client has no reason to resync.
                    assert!(
                        is_error(&told[ME], ErrorCode::BadMessage),
                        "{side:?} {step:?}: {:?}",
                        told[ME]
                    );
                    let told = poke(&handle, &mut clients, ME, set(target, false)).await;
                    assert!(
                        is_error(&told[ME], ErrorCode::BadMessage),
                        "clearing {side:?} {step:?} is refused too"
                    );
                    silent(&told[FOE]);
                }
            }
        }
    }
    let held = reconnect(&handle, &mut clients[ME]).await;
    assert_eq!(held, (Vec::new(), FullControlMode::Off), "nothing was kept");
}

#[rstest]
#[tokio::test]
async fn setting_a_held_stop_again_is_still_one_stop_and_a_request_that_changes_nothing_is_silent(
    #[values(false, true)] started: bool,
    #[values(false, true)] keep: bool,
) {
    let (handle, _, mut clients) = table(instant_duel(), 2, started).await;
    let held = stop(StopSide::Own, Step::Main1, keep);

    let told = poke(&handle, &mut clients, ME, set(held, true)).await;
    assert_eq!(stops_told(&told[ME]), [vec![held]]);
    // The same again, however often: nothing changed, so nothing is said.
    for _ in 0..3 {
        let told = poke(&handle, &mut clients, ME, set(held, true)).await;
        silent(&told[ME]);
        silent(&told[FOE]);
    }
    assert_eq!(
        reconnect(&handle, &mut clients[ME]).await.0,
        [held],
        "one stop, not four"
    );

    // Clearing ignores the `keep` the request carries, so a request with the other one clears it as well.
    let other = stop(held.side, held.step, !keep);
    let told = poke(&handle, &mut clients, ME, set(other, false)).await;
    assert_eq!(stops_told(&told[ME]), [Vec::<Stop>::new()]);
    for request in [held, other] {
        let told = poke(&handle, &mut clients, ME, set(request, false)).await;
        silent(&told[ME]);
    }
}

#[rstest]
#[tokio::test]
async fn a_lasting_and_a_one_turn_stop_are_told_with_the_keep_each_was_set_with(
    #[values(false, true)] started: bool,
) {
    let (handle, _, mut clients) = table(instant_duel(), 2, started).await;
    let lasting = stop(StopSide::Own, Step::Main1, true);
    let one_turn = stop(StopSide::Opponent, Step::End, false);

    let told = poke(&handle, &mut clients, ME, set(one_turn, true)).await;
    assert_eq!(stops_told(&told[ME]), [vec![one_turn]]);
    let told = poke(&handle, &mut clients, ME, set(lasting, true)).await;
    assert_eq!(stops_told(&told[ME]), [vec![lasting, one_turn]]);
    silent(&told[FOE]);

    // A welcome says the same, and another seat holds neither.
    let (held, mode) = reconnect(&handle, &mut clients[ME]).await;
    assert_eq!(
        (held, mode),
        (vec![lasting, one_turn], FullControlMode::Off)
    );
    assert_eq!(reconnect(&handle, &mut clients[FOE]).await.0, []);
}

#[rstest]
#[tokio::test]
async fn setting_a_held_stop_again_with_another_keep_upgrades_or_downgrades_it_and_tells_the_list(
    #[values(false, true)] started: bool,
) {
    let (handle, _, mut clients) = table(instant_duel(), 2, started).await;
    let at = |keep| stop(StopSide::Own, Step::Main1, keep);
    let other = stop(StopSide::Opponent, Step::Draw, true);
    poke(&handle, &mut clients, ME, set(other, true)).await;

    // One turn, then lasting, then one turn again, with a repeat in between that says nothing.
    let requests = [
        (at(false), Some(vec![at(false), other])),
        (at(true), Some(vec![at(true), other])),
        (at(true), None),
        (at(false), Some(vec![at(false), other])),
        (at(false), None),
        (at(true), Some(vec![at(true), other])),
    ];
    for (request, told_now) in requests {
        let told = poke(&handle, &mut clients, ME, set(request, true)).await;
        assert_eq!(
            stops_told(&told[ME]),
            told_now.clone().into_iter().collect::<Vec<_>>(),
            "{request:?}"
        );
        assert_eq!(
            told[ME].len(),
            usize::from(told_now.is_some()),
            "only the list is told"
        );
        silent(&told[FOE]);
    }
    // Where it ended up is what a welcome says.
    assert_eq!(
        reconnect(&handle, &mut clients[ME]).await.0,
        [at(true), other]
    );
}

#[tokio::test]
async fn the_whole_list_is_told_each_time_own_side_first_and_each_side_in_turn_order_whatever_the_keeps()
 {
    let (handle, _, mut clients) = table(instant_duel(), 2, false).await;
    let (a, b, c, d) = (
        stop(StopSide::Opponent, Step::End, true),
        stop(StopSide::Own, Step::Main2, false),
        stop(StopSide::Own, Step::Upkeep, true),
        stop(StopSide::Opponent, Step::Upkeep, false),
    );
    let mut told = Vec::new();
    for item in [a, b, c, d] {
        let got = poke(&handle, &mut clients, ME, set(item, true)).await;
        told.extend(stops_told(&got[ME]));
    }
    assert_eq!(told, [vec![a], vec![b, a], vec![c, b, a], vec![c, b, d, a]]);

    // Turning two of them over changes their keeps and moves nothing.
    let flipped = |s: Stop| stop(s.side, s.step, !s.keep);
    let got = poke(&handle, &mut clients, ME, set(flipped(b), true)).await;
    assert_eq!(stops_told(&got[ME]), [vec![c, flipped(b), d, a]]);
    let got = poke(&handle, &mut clients, ME, set(flipped(d), true)).await;
    assert_eq!(stops_told(&got[ME]), [vec![c, flipped(b), flipped(d), a]]);
    assert_eq!(
        reconnect(&handle, &mut clients[ME]).await.0,
        [c, flipped(b), flipped(d), a],
        "and a welcome lists them alike"
    );
}

#[rstest]
#[tokio::test]
async fn each_seat_holds_its_own_and_only_it_is_told(#[values(false, true)] started: bool) {
    let (handle, _, mut clients) = table(table_setup(3, 5), 3, started).await;
    let mine = stop(StopSide::Own, Step::Upkeep, true);
    let told = poke(&handle, &mut clients, 2, set(mine, true)).await;
    assert_eq!(stops_told(&told[2]), [vec![mine]]);
    silent(&told[0]);
    silent(&told[1]);
    let told = poke(
        &handle,
        &mut clients,
        2,
        full_control(FullControlMode::Held),
    )
    .await;
    assert_eq!(modes_told(&told[2]), [FullControlMode::Held]);
    silent(&told[0]);
    silent(&told[1]);

    // A reconnecting seat is told what the server holds for it, and only for it.
    let nothing = (Vec::new(), FullControlMode::Off);
    assert_eq!(
        reconnect(&handle, &mut clients[2]).await,
        (vec![mine], FullControlMode::Held)
    );
    assert_eq!(reconnect(&handle, &mut clients[1]).await, nothing);
    assert_eq!(reconnect(&handle, &mut clients[0]).await, nothing);
}

#[tokio::test]
async fn full_control_takes_each_mode_and_a_mode_it_already_has_says_nothing() {
    use FullControlMode::{Held, NextWindow, Off};
    let (handle, _, mut clients) = table(instant_duel(), 2, false).await;
    let requests = [
        (Off, None),
        (NextWindow, Some(NextWindow)),
        (NextWindow, None),
        (Held, Some(Held)),
        (NextWindow, Some(NextWindow)),
        (Off, Some(Off)),
        (Off, None),
        (Held, Some(Held)),
    ];
    for (mode, told_now) in requests {
        let told = poke(&handle, &mut clients, ME, full_control(mode)).await;
        assert_eq!(
            modes_told(&told[ME]),
            told_now.into_iter().collect::<Vec<_>>(),
            "{mode:?}"
        );
        assert_eq!(
            told[ME].len(),
            usize::from(told_now.is_some()),
            "only the mode is told"
        );
        silent(&told[FOE]);
    }
    assert_eq!(reconnect(&handle, &mut clients[ME]).await.1, Held);
}

#[tokio::test]
async fn they_survive_a_reconnect_and_a_resync_and_nothing_is_repeated_to_the_seat() {
    let (handle, _, mut clients) = table(instant_duel(), 2, true).await;
    let own = stop(StopSide::Own, Step::Upkeep, true);
    let theirs = stop(StopSide::Opponent, Step::DeclareAttackers, false);
    poke(&handle, &mut clients, ME, set(own, true)).await;
    poke(&handle, &mut clients, ME, set(theirs, true)).await;
    poke(
        &handle,
        &mut clients,
        ME,
        full_control(FullControlMode::NextWindow),
    )
    .await;

    // A new socket for the seat: the welcome states everything and no notice follows it.
    let held = reconnect(&handle, &mut clients[ME]).await;
    assert_eq!(held, (vec![own, theirs], FullControlMode::NextWindow));
    let rest = clients[ME].drain();
    assert_eq!(order(&rest), ["snapshot"], "seat 1 has no decision pending");

    // A resync repeats the snapshot and the decision, not the settings.
    handle.send(seat(ME), ClientMessage::Resync).await.unwrap();
    handle.status().await.unwrap();
    assert_eq!(order(&clients[ME].drain()), ["snapshot"]);
    let held = reconnect(&handle, &mut clients[ME]).await;
    assert_eq!(held, (vec![own, theirs], FullControlMode::NextWindow));
}

#[tokio::test]
async fn stops_set_in_the_lobby_are_held_for_the_game() {
    let (handle, _, mut clients) = table(instant_duel(), 2, false).await;
    let held = stop(StopSide::Opponent, Step::Main1, true);
    let told = poke(&handle, &mut clients, FOE, set(held, true)).await;
    assert_eq!(stops_told(&told[FOE]), [vec![held]]);
    poke(
        &handle,
        &mut clients,
        FOE,
        full_control(FullControlMode::Held),
    )
    .await;

    for client in &clients {
        client.say(&handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    assert!(handle.status().await.unwrap().started);
    let started = clients[FOE].drain();
    assert!(stops_told(&started).is_empty() && modes_told(&started).is_empty());
    let kept = reconnect(&handle, &mut clients[FOE]).await;
    assert_eq!(kept, (vec![held], FullControlMode::Held));
}

#[tokio::test]
async fn a_game_resumed_from_its_log_starts_without_stops() {
    let setup = instant_duel();
    let store = MemoryStore::new();
    let first = spawn_game_with(game_id(1), setup.clone(), store.clone(), windowed()).unwrap();
    let mut clients = vec![
        Client::join(&first, FOE).await,
        Client::join(&first, ME).await,
    ];
    for client in &clients {
        client.say(&first, ClientMessage::Ready).await;
    }
    let held = stop(StopSide::Own, Step::Main1, true);
    poke(&first, &mut clients, ME, set(held, true)).await;
    poke(
        &first,
        &mut clients,
        ME,
        full_control(FullControlMode::Held),
    )
    .await;
    let before = reconnect(&first, &mut clients[ME]).await;
    assert_eq!(before, (vec![held], FullControlMode::Held));
    drop(clients);
    drop(first);

    let second = resume_game_with(store, game_id(1), setup, windowed())
        .await
        .unwrap();
    let mut me = Client::join(&second, ME).await;
    let ServerMessage::Welcome {
        stops,
        full_control,
        ..
    } = me.next().await
    else {
        panic!("the first message is the welcome");
    };
    assert_eq!((stops, full_control), (Vec::new(), FullControlMode::Off));
}

#[tokio::test]
async fn setting_them_takes_no_step_so_nothing_is_saved_and_the_pending_decision_stays_answerable()
{
    let (handle, store, mut clients) = table(instant_duel(), 2, true).await;
    // Seat 0 holds priority in its first main phase.
    let (id, decision) = clients[FOE].decision().await;
    let before = handle.status().await.unwrap();
    let logged = store.inner().load(game_id(1)).await.unwrap();

    let mine = stop(StopSide::Own, Step::Main1, true);
    let theirs = stop(StopSide::Opponent, Step::End, false);
    for (from, message) in [
        (ME, set(mine, true)),
        (FOE, set(theirs, true)),
        (ME, full_control(FullControlMode::Held)),
        (FOE, full_control(FullControlMode::NextWindow)),
    ] {
        let told = poke(&handle, &mut clients, from, message).await;
        assert_eq!(
            told[from].len(),
            1,
            "one notice and no decision or events: {:?}",
            told[from]
        );
        silent(&told[1 - from]);
    }

    assert_eq!(
        handle.status().await.unwrap(),
        before,
        "the game did not move"
    );
    assert_eq!(store.inner().load(game_id(1)).await.unwrap(), logged);

    // Seat 0 still holds the decision it was sent before, under the same id, and can use it.
    let answer = ClientMessage::Answer {
        decision: id,
        answer: passive(&decision),
    };
    let told = poke(&handle, &mut clients, FOE, answer).await;
    assert!(
        !told[FOE]
            .iter()
            .any(|m| matches!(m, ServerMessage::Error { .. })),
        "{:?}",
        told[FOE]
    );
    assert!(handle.status().await.unwrap().events > before.events);
}

#[tokio::test]
async fn a_seat_that_concedes_loses_what_it_held_is_told_after_the_events_and_cannot_set_more() {
    let (handle, _, mut clients) = table(table_setup(3, 5), 3, true).await;
    let held = stop(StopSide::Own, Step::Main1, true);
    poke(&handle, &mut clients, 2, set(held, true)).await;
    poke(
        &handle,
        &mut clients,
        2,
        full_control(FullControlMode::Held),
    )
    .await;
    poke(&handle, &mut clients, 0, set(held, true)).await;

    let told = poke(&handle, &mut clients, 2, ClientMessage::Concede).await;
    assert_eq!(order(&told[2]), ["events", "stops", "full control"]);
    assert_eq!(stops_told(&told[2]), [Vec::<Stop>::new()]);
    assert_eq!(modes_told(&told[2]), [FullControlMode::Off]);
    assert!(stops_told(&told[0]).is_empty() && stops_told(&told[1]).is_empty());
    assert_eq!(
        reconnect(&handle, &mut clients[0]).await.0,
        [held],
        "the others keep theirs"
    );

    for message in [
        set(stop(StopSide::Own, Step::Upkeep, false), true),
        set(stop(StopSide::Own, Step::Upkeep, false), false),
        full_control(FullControlMode::Held),
        full_control(FullControlMode::Off),
    ] {
        let told = poke(&handle, &mut clients, 2, message).await;
        assert!(is_error(&told[2], ErrorCode::BadMessage), "{:?}", told[2]);
    }
    let nothing = (Vec::new(), FullControlMode::Off);
    assert_eq!(reconnect(&handle, &mut clients[2]).await, nothing);
}

#[tokio::test]
async fn when_the_game_ends_every_seat_is_told_before_the_game_over_and_can_set_no_more() {
    let (handle, _, mut clients) = table(instant_duel(), 2, true).await;
    for seat in [FOE, ME] {
        let message = set(stop(StopSide::Opponent, Step::End, true), true);
        poke(&handle, &mut clients, seat, message).await;
        poke(
            &handle,
            &mut clients,
            seat,
            full_control(FullControlMode::Held),
        )
        .await;
    }

    // With two seats a concession ends the game.
    let told = poke(&handle, &mut clients, ME, ClientMessage::Concede).await;
    assert_eq!(
        handle.status().await.unwrap().result,
        Some(GameResult::Winner(PlayerId(0)))
    );
    for seat in [FOE, ME] {
        let shape = order(&told[seat]);
        assert_eq!(
            shape,
            ["events", "stops", "full control", "game over"],
            "seat {seat}"
        );
        assert_eq!(stops_told(&told[seat]), [Vec::<Stop>::new()]);
        assert_eq!(modes_told(&told[seat]), [FullControlMode::Off]);
    }
    for seat in [FOE, ME] {
        for message in [
            full_control(FullControlMode::Held),
            set(stop(StopSide::Own, Step::End, true), true),
        ] {
            let told = poke(&handle, &mut clients, seat, message).await;
            assert!(
                is_error(&told[seat], ErrorCode::BadMessage),
                "the game is over: {:?}",
                told[seat]
            );
        }
        let nothing = (Vec::new(), FullControlMode::Off);
        assert_eq!(reconnect(&handle, &mut clients[seat]).await, nothing);
    }
}

#[tokio::test]
async fn a_failed_save_leaves_what_the_seat_held_and_the_retry_clears_it() {
    let (handle, store, mut clients) = table(table_setup(3, 5), 3, true).await;
    let held = stop(StopSide::Opponent, Step::End, false);
    poke(&handle, &mut clients, 2, set(held, true)).await;
    poke(
        &handle,
        &mut clients,
        2,
        full_control(FullControlMode::NextWindow),
    )
    .await;
    let before = handle.status().await.unwrap();

    // The concession cannot be saved: the game and the seat's settings stay as they were, and nobody hears of it.
    store.fail(true);
    let told = poke(&handle, &mut clients, 2, ClientMessage::Concede).await;
    assert!(is_error(&told[2], ErrorCode::Internal), "{:?}", told[2]);
    silent(&told[0]);
    silent(&told[1]);
    assert_eq!(handle.status().await.unwrap(), before);
    let kept = reconnect(&handle, &mut clients[2]).await;
    assert_eq!(kept, (vec![held], FullControlMode::NextWindow));

    // Setting them needs no save, so the seat can still change them; once the store works the same concession
    // clears them and says so.
    let told = poke(
        &handle,
        &mut clients,
        2,
        full_control(FullControlMode::Held),
    )
    .await;
    assert_eq!(modes_told(&told[2]), [FullControlMode::Held]);
    store.fail(false);
    let told = poke(&handle, &mut clients, 2, ClientMessage::Concede).await;
    assert_eq!(stops_told(&told[2]), [Vec::<Stop>::new()]);
    assert_eq!(modes_told(&told[2]), [FullControlMode::Off]);
    let nothing = (Vec::new(), FullControlMode::Off);
    assert_eq!(reconnect(&handle, &mut clients[2]).await, nothing);
}

/// Plays a whole game of bots in which every seat holds every stop and Full Control from the lobby on.
async fn bots_that_hold_everything(players: usize, seed: u64) {
    let setup = table_setup(players, seed);
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(1), setup.clone(), store.clone(), roomy()).unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        bots.push(Bot::join(&handle, n, seed).await);
    }
    for n in 0..players {
        for side in SIDES {
            for step in Step::ALL.into_iter().filter(|step| step.grants_priority()) {
                let message = set(stop(side, step, n % 2 == 0), true);
                handle.send(seat(n), message).await.unwrap();
            }
        }
        handle
            .send(seat(n), full_control(FullControlMode::Held))
            .await
            .unwrap();
    }
    let reports = common::play_bots(&handle, bots).await;

    // Nothing reads them: the log is the one the same bots write on a bare engine.
    let log = store.load(game_id(1)).await.unwrap();
    let straight = common::reference(players, seed);
    common::assert_same_game(&log, &straight.events);
    let status = handle.status().await.unwrap();
    assert_eq!(status.state_hash, Some(straight.final_hash));
    assert_eq!(
        status.state_hash,
        Some(replay(&setup, &log).unwrap().state_hash())
    );
    assert!(reports.iter().all(|r| r.result == straight.result));
    assert!(reports.iter().all(|r| r.errors.is_empty()), "{reports:?}");
    // The game is over, so every seat holds nothing again.
    for n in 0..players {
        let mut client = Client::join(&handle, n).await;
        let ServerMessage::Welcome {
            stops,
            full_control,
            ..
        } = client.next().await
        else {
            panic!("the first message is the welcome");
        };
        assert_eq!(
            (stops, full_control),
            (Vec::new(), FullControlMode::Off),
            "seat {n}"
        );
    }
}

#[rstest]
#[tokio::test]
async fn a_game_where_every_seat_holds_every_stop_and_full_control_is_the_same_game(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3)] seed: u64,
) {
    bots_that_hold_everything(players, seed).await;
}
