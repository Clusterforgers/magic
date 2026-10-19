//! Goal: a seat that yields is passed for through the normal path at every priority window until its yield ends (an opponent's new object, whatever the seat could do about it, the end of the turn, its own turn, the stack empty), is told when the yield starts and ends and is never sent a priority decision in between, still gets every other decision, and loses the yield with a cancel, a concession, a lost game, a resume or a failed save; the pending decision is passed at once or sent again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Bot, Client, FlakyStore, game_id, passive, roomy, seat, silent, table_setup};
use mtg_core::ids::PlayerId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{DecisionKind, PayChoice, PriorityAction};
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Decision, Event, GameSetup, replay};
use mtg_protocol::{
    ClientMessage, DecisionId, ErrorCode, FullControlMode, ServerMessage, YieldUntil,
};
use mtg_server::{
    Dwell, EventStore, GameConfig, GameHandle, MemoryStore, Pace, resume_game_with, spawn_game_with,
};
use mtg_testkit::cards::{forest, free_creature, instant};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

/// The default windows decide when a seat is asked, and no clock runs. A seat that yields is in Full Control `Off`;
/// `join_and_ready` puts seat 0, which plays by hand, in Full Control so that it is asked at every window.
const OFF: Pace = Pace {
    auto_pass: true,
    response: None,
    blocks: None,
    dwell: Dwell::OFF,
};

/// The server defaults: auto-pass on, 10 s to respond, 45 s to block.
const DEFAULT: Pace = Pace {
    auto_pass: true,
    response: Some(Duration::from_secs(10)),
    blocks: Some(Duration::from_secs(45)),
    dwell: Dwell::OFF,
};

const FOE: usize = 0;
const ME: usize = 1;

fn config(pace: Pace) -> GameConfig {
    GameConfig { pace, ..roomy() }
}

/// Seat 0 (the active player, in its first main phase) holds two free instants and seat 1 one: both can act in
/// every window, even over a spell on the stack.
fn instant_duel() -> GameSetup {
    Scenario::new(2)
        .hand(
            0,
            [instant("Foe Zap 0", 0, &[]), instant("Foe Zap 1", 0, &[])],
        )
        .hand(1, [instant("My Zap", 0, &[])])
        .setup()
        .unwrap()
}

/// Seat 0 holds a creature it casts in its main phase and seat 1 holds nothing, so it can never respond.
fn creature_duel() -> GameSetup {
    Scenario::new(2)
        .hand(0, [free_creature("Raider", 2, 2)])
        .setup()
        .unwrap()
}

/// Turn 2, seat 1's own first main phase, with a free instant in hand: a default window over an empty stack.
fn my_main() -> GameSetup {
    Scenario::new(2)
        .at(2, 1, Step::Main1)
        .hand(1, [instant("My Zap", 0, &[])])
        .setup()
        .unwrap()
}

/// Seat 0 is about to attack with a creature that seat 1 can block; seat 1 holds a land for its own next turn and a
/// free instant.
fn combat_duel() -> GameSetup {
    Scenario::new(2)
        .hand(1, [forest(), instant("My Zap", 0, &[])])
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Raider", 2, 2))])
        .battlefield(1, [ready(free_creature("Guard", 1, 3))])
        .setup()
        .unwrap()
}

fn spawn(setup: GameSetup, pace: Pace) -> (GameHandle, FlakyStore) {
    let store = FlakyStore::new();
    let handle = spawn_game_with(game_id(1), setup, store.clone(), config(pace)).unwrap();
    (handle, store)
}

/// Seats `seats` clients and readies them all; seat 0 first asks for Full Control, so the hand-driven seat is asked at
/// every window while the others are asked as the windows say.
async fn join_and_ready(handle: &GameHandle, seats: usize) -> Vec<Client> {
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(handle, n).await);
    }
    let held = ClientMessage::FullControl {
        mode: FullControlMode::Held,
    };
    clients[FOE].say(handle, held).await;
    for client in &clients {
        client.say(handle, ClientMessage::Ready).await;
    }
    handle.status().await.unwrap();
    clients
}

/// Sends `message` as `from` and waits until the actor has dealt with it.
async fn send(handle: &GameHandle, from: &Client, message: ClientMessage) {
    from.say(handle, message).await;
    handle.status().await.unwrap();
}

async fn yield_until(handle: &GameHandle, from: &Client, until: YieldUntil) {
    send(handle, from, ClientMessage::Yield { until }).await;
}

async fn answer(handle: &GameHandle, from: &Client, id: DecisionId, answer: Answer) {
    let message = ClientMessage::Answer {
        decision: id,
        answer,
    };
    send(handle, from, message).await;
}

/// Answers passively for `seat` each time the game waits on it; returns the answers given.
async fn pass_while_waiting(handle: &GameHandle, client: &mut Client) -> usize {
    let mut given = 0;
    while handle.status().await.unwrap().waiting_on == Some(client.seat) {
        let (id, decision) = client.decision().await;
        answer(handle, client, id, passive(&decision)).await;
        given += 1;
    }
    given
}

/// Seat 0 casts the first spell it can and pays for it; it holds priority again afterwards.
async fn foe_casts(handle: &GameHandle, foe: &mut Client) {
    let (id, decision) = foe.decision().await;
    foe_casts_at(handle, foe, id, &decision).await;
}

/// [`foe_casts`] at the window `id` that seat 0 was already sent.
async fn foe_casts_at(handle: &GameHandle, foe: &mut Client, id: DecisionId, decision: &Decision) {
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("expected priority, got {decision:?}");
    };
    let spell = options.castable[0];
    answer(
        handle,
        foe,
        id,
        Answer::Priority(PriorityAction::CastSpell(spell)),
    )
    .await;
    let (id, decision) = foe.decision().await;
    assert_eq!(decision.kind.name(), "pay-mana");
    answer(handle, foe, id, Answer::PayMana(PayChoice::Auto)).await;
}

/// Seat 0 passes once.
async fn foe_passes_once(handle: &GameHandle, foe: &mut Client) {
    let (id, decision) = foe.decision().await;
    answer(handle, foe, id, passive(&decision)).await;
}

/// A message as the order tests compare it.
fn shape(message: &ServerMessage) -> String {
    match message {
        ServerMessage::Welcome { .. } => "welcome".into(),
        ServerMessage::Lobby { .. } => "lobby".into(),
        ServerMessage::Decision { decision, .. } => format!("decision {}", decision.kind.name()),
        ServerMessage::Events(_) => "events".into(),
        ServerMessage::Snapshot { .. } => "snapshot".into(),
        ServerMessage::Chat { .. } => "chat".into(),
        ServerMessage::Error { code, .. } => format!("error {code:?}"),
        ServerMessage::GameOver { .. } => "game over".into(),
        ServerMessage::Forecast { .. } => "forecast".into(),
        ServerMessage::Yielding { until: Some(until) } => format!("yield {until:?}"),
        ServerMessage::Yielding { until: None } => "yield none".into(),
        ServerMessage::Stops { .. } => "stops".into(),
        ServerMessage::FullControl { .. } => "full control".into(),
    }
}

/// The messages in order, with a run of event batches counted once.
fn order(messages: &[ServerMessage]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for message in messages {
        let now = shape(message);
        if now != "events" || out.last().is_none_or(|last| last != "events") {
            out.push(now);
        }
    }
    out
}

fn notices(messages: &[ServerMessage]) -> Vec<Option<YieldUntil>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Yielding { until } => Some(*until),
            _ => None,
        })
        .collect()
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

fn priority_decisions(messages: &[ServerMessage]) -> usize {
    decisions(messages)
        .iter()
        .filter(|(_, d)| matches!(d.kind, DecisionKind::Priority(_)))
        .count()
}

/// The turn number and stack size of the last event batch among `messages`.
fn last_table(messages: &[ServerMessage]) -> (u32, u32) {
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            ServerMessage::Events(batch) => {
                Some((batch.table.turn.number, batch.table.stack.count))
            }
            _ => None,
        })
        .expect("an event batch")
}

fn deadline(messages: &[ServerMessage]) -> Option<u64> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::Decision { deadline_ms, .. } => *deadline_ms,
        _ => None,
    })
}

fn is_error(messages: &[ServerMessage], expected: ErrorCode) -> bool {
    matches!(messages, [ServerMessage::Error { code, .. }] if *code == expected)
}

fn passes_by(log: &[Event], player: usize) -> usize {
    log.iter()
        .filter(|e| matches!(e, Event::PriorityPassed { player: p } if p.index() == player))
        .count()
}

async fn logged(store: &FlakyStore) -> Vec<Event> {
    store.inner().load(game_id(1)).await.unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_yield_to_the_end_of_turn_passes_at_every_window_and_returns_the_first_one_of_the_next_turn()
 {
    let (handle, store) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    clients[ME].drain();

    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    assert_eq!(notices(&clients[ME].drain()), [Some(YieldUntil::EndOfTurn)]);

    // Seat 0 plays on through the rest of its turn; seat 1 holds a spell and could answer every window.
    let windows = pass_while_waiting(&handle, &mut clients[FOE]).await;
    assert!(windows >= 5, "seat 0 went through {windows} windows");
    assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(ME)));

    let told = clients[ME].drain();
    assert_eq!(
        priority_decisions(&told),
        1,
        "only the window that ended it"
    );
    // The yield ended in seat 1's upkeep of turn 2, which is no window for it, and the notice went out with that
    // step; the first window it is asked at is its own first main phase.
    let tail: Vec<_> = order(&told).into_iter().rev().take(3).collect();
    assert_eq!(tail, ["decision priority", "events", "yield none"]);
    assert_eq!(last_table(&told).0, 2, "its first main phase of turn 2");
    // It was passed for in the windows it skipped, as a normal pass in the log.
    assert!(passes_by(&logged(&store).await, ME) >= 5);
    // The yield is gone: the seat answers its own window and the next one is delivered to it.
    let (id, decision) = decisions(&told).remove(0);
    answer(&handle, &clients[ME], id, passive(&decision)).await;
    let next = clients[ME].drain();
    assert!(
        next.iter()
            .all(|m| !matches!(m, ServerMessage::Yielding { .. }))
    );
}

#[tokio::test(start_paused = true)]
async fn setting_a_yield_passes_the_pending_decision_at_once_and_the_old_id_goes_stale() {
    let (handle, store) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    foe_casts(&handle, &mut clients[FOE]).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;
    let (window, decision) = clients[ME].decision().await;
    assert_eq!(decision.kind.name(), "priority");
    clients[ME].drain();
    let passes = passes_by(&logged(&store).await, ME);

    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    let told = clients[ME].drain();
    assert_eq!(order(&told), ["events", "yield EndOfTurn"]);
    assert_eq!(passes_by(&logged(&store).await, ME), passes + 1);
    assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(FOE)));

    answer(&handle, &clients[ME], window, passive(&decision)).await;
    assert!(is_error(&clients[ME].drain(), ErrorCode::StaleDecision));
}

/// The duel in which seat 1 holds an instant to answer a spell with, or the one in which it holds nothing.
fn duel_for(can_answer: bool) -> GameSetup {
    if can_answer {
        instant_duel()
    } else {
        creature_duel()
    }
}

#[rstest]
#[tokio::test(start_paused = true)]
async fn an_opponents_new_spell_ends_a_yield_whether_or_not_the_seat_could_answer_it_and_the_window_is_the_seats_only_if_it_could(
    #[values(YieldUntil::EndOfTurn, YieldUntil::MyTurn)] until: YieldUntil,
    #[values(true, false)] can_answer: bool,
) {
    let (handle, _) = spawn(
        duel_for(can_answer),
        Pace {
            response: Some(Duration::from_secs(10)),
            ..OFF
        },
    );
    let mut clients = join_and_ready(&handle, 2).await;
    yield_until(&handle, &clients[ME], until).await;
    clients[ME].drain();

    foe_casts(&handle, &mut clients[FOE]).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;

    let told = clients[ME].drain();
    assert_eq!(notices(&told), [None], "the yield ends either way");
    if can_answer {
        // The spell is a default window for a seat that can act, on the usual clock.
        assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(ME)));
        assert_eq!(priority_decisions(&told), 1);
        let tail: Vec<_> = order(&told).into_iter().rev().take(2).collect();
        assert_eq!(tail, ["decision priority", "yield none"]);
        assert_eq!(last_table(&told), (1, 1), "turn 1, the spell on the stack");
        assert_eq!(deadline(&told), Some(10_000), "and the usual clock starts");
        let (_, decision) = decisions(&told).remove(0);
        let DecisionKind::Priority(options) = decision.kind else {
            panic!("expected priority");
        };
        assert!(options.can_act);
    } else {
        // Nothing to answer with: the window is passed for the seat and the spell resolves.
        assert_eq!(priority_decisions(&told), 0);
        assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(FOE)));
        assert_eq!(
            last_table(&told),
            (1, 0),
            "turn 1, the stack is empty again"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_cannot_act_is_passed_for_at_the_window_that_ended_its_yield_and_auto_pass_on_changes_nothing()
 {
    // Seat 1 holds nothing. The spell ends its yield and is announced; the window is then the table's to pass.
    let (handle, _) = spawn(creature_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    clients[ME].drain();
    send(
        &handle,
        &clients[ME],
        ClientMessage::AutoPass { enabled: true },
    )
    .await;
    silent(&clients[ME].drain());
    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    clients[ME].drain();

    foe_casts(&handle, &mut clients[FOE]).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;

    let told = clients[ME].drain();
    assert_eq!(notices(&told), [None]);
    assert_eq!(priority_decisions(&told), 0);
    assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(FOE)));
}

#[rstest]
#[case::end_of_turn(YieldUntil::EndOfTurn)]
#[case::my_turn(YieldUntil::MyTurn)]
#[case::resolve(YieldUntil::Resolve)]
#[tokio::test(start_paused = true)]
async fn a_spell_the_seat_already_saw_does_not_end_the_yield_it_asks_over_it(
    #[case] until: YieldUntil,
) {
    let (handle, _) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    foe_casts(&handle, &mut clients[FOE]).await;
    // The spell is on the stack and seat 1 is shown it, as it would be without a yield.
    foe_passes_once(&handle, &mut clients[FOE]).await;
    let (_, decision) = clients[ME].decision().await;
    assert_eq!(decision.kind.name(), "priority");
    clients[ME].drain();

    yield_until(&handle, &clients[ME], until).await;
    let after_yield = clients[ME].drain();
    assert_eq!(notices(&after_yield), [Some(until)]);
    // The spell resolves and every yield is over by the end of seat 0's turn (`Resolve` at once). Seat 1 is asked
    // again at its own first main phase, the first window it can use, in turn 2.
    pass_while_waiting(&handle, &mut clients[FOE]).await;
    let told = clients[ME].drain();
    assert_eq!(notices(&told), [None]);
    assert_eq!(priority_decisions(&told), 1);
    assert_eq!(last_table(&told).0, 2);
}

#[tokio::test(start_paused = true)]
async fn resolve_passes_until_the_stack_is_empty_and_then_leaves_the_seat_to_the_windows() {
    let (handle, _) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    foe_casts(&handle, &mut clients[FOE]).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;
    let (_, decision) = clients[ME].decision().await;
    assert_eq!(decision.kind.name(), "priority");
    clients[ME].drain();

    yield_until(&handle, &clients[ME], YieldUntil::Resolve).await;
    // Its pass completes the round, the spell resolves and seat 0 holds priority over an empty stack.
    assert_eq!(order(&clients[ME].drain()), ["events", "yield Resolve"]);
    assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(FOE)));

    // Seat 1's next window has an empty stack under seat 0's main phase, which is no window for it: its yield is
    // over, the notice says so and the window is passed for it.
    foe_passes_once(&handle, &mut clients[FOE]).await;
    let told = clients[ME].drain();
    assert_eq!(order(&told), ["events", "yield none"]);
    assert_eq!(last_table(&told), (1, 0), "the stack is empty");
    assert_eq!(handle.status().await.unwrap().waiting_on, Some(seat(FOE)));
}

#[tokio::test(start_paused = true)]
async fn resolve_also_ends_for_a_new_spell_of_the_opponent_while_it_waits() {
    let (handle, _) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    foe_casts(&handle, &mut clients[FOE]).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;
    clients[ME].decision().await;
    yield_until(&handle, &clients[ME], YieldUntil::Resolve).await;
    clients[ME].drain();

    // The first spell resolved; seat 0 answers with another before seat 1 gets a window.
    foe_casts(&handle, &mut clients[FOE]).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;
    let told = clients[ME].drain();
    assert_eq!(order(&told), ["events", "yield none", "decision priority"]);
    assert_eq!(last_table(&told), (1, 1), "a new spell is on the stack");
}

#[tokio::test(start_paused = true)]
async fn resolve_over_an_empty_stack_sends_the_window_again_with_a_new_id() {
    let (handle, store) = spawn(my_main(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    let (window, decision) = clients[ME].decision().await;
    clients[ME].drain();
    let events = handle.status().await.unwrap().events;

    yield_until(&handle, &clients[ME], YieldUntil::Resolve).await;
    let told = clients[ME].drain();
    assert_eq!(order(&told), ["decision priority"], "no yield, no notice");
    let (again, same) = decisions(&told).remove(0);
    assert!(again > window);
    assert_eq!(same, decision);
    assert_eq!(
        handle.status().await.unwrap().events,
        events,
        "nothing was passed"
    );
    assert_eq!(logged(&store).await.len() as u64, events);

    answer(&handle, &clients[ME], window, passive(&decision)).await;
    assert!(is_error(&clients[ME].drain(), ErrorCode::StaleDecision));
    answer(&handle, &clients[ME], again, passive(&decision)).await;
    assert!(
        decisions(&clients[ME].drain()).is_empty()
            || handle.status().await.unwrap().events > events
    );
}

#[tokio::test(start_paused = true)]
async fn a_decision_that_is_not_priority_is_delivered_and_the_yield_goes_on() {
    let (handle, store) = spawn(combat_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    clients[ME].drain();

    let (id, decision) = clients[FOE].decision().await;
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("expected the attack, got {decision:?}");
    };
    let attack = vec![(
        options.attackers[0].object,
        AttackTarget::Player(PlayerId(1)),
    )];
    answer(&handle, &clients[FOE], id, Answer::DeclareAttackers(attack)).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;

    // The blocker is asked in spite of the yield, which stays.
    let told = clients[ME].drain();
    assert_eq!(order(&told), ["events", "decision declare-blockers"]);
    assert_eq!(notices(&told), []);
    let (blocks, _) = decisions(&told).remove(0);
    answer(
        &handle,
        &clients[ME],
        blocks,
        Answer::DeclareBlockers(Vec::new()),
    )
    .await;

    // The rest of the turn is passed for, up to the first window of turn 3, which belongs to seat 1.
    pass_while_waiting(&handle, &mut clients[FOE]).await;
    let told = clients[ME].drain();
    assert_eq!(priority_decisions(&told), 1);
    assert_eq!(notices(&told), [None]);
    assert_eq!(last_table(&told).0, 3);
    assert!(passes_by(&logged(&store).await, ME) >= 3);
}

#[tokio::test(start_paused = true)]
async fn a_discard_at_the_end_of_the_turn_is_delivered_to_a_seat_that_yields() {
    // Seat 1's hand is over the limit when its own turn ends; it yields until the end of that turn.
    let setup = Scenario::new(2)
        .at(1, 1, Step::Main1)
        .hand(1, vec![instant("Spare", 0, &[]); 9])
        .setup()
        .unwrap();
    let (handle, _) = spawn(setup, OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    let (_, first) = clients[ME].decision().await;
    assert_eq!(first.kind.name(), "priority");
    clients[ME].drain();

    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    pass_while_waiting(&handle, &mut clients[FOE]).await;
    let told = clients[ME].drain();
    let (_, discard) = decisions(&told).pop().expect("a decision");
    assert_eq!(discard.kind.name(), "discard");
    assert_eq!(
        notices(&told),
        [Some(YieldUntil::EndOfTurn)],
        "still yielding"
    );
    assert_eq!(priority_decisions(&told), 0);
}

#[tokio::test(start_paused = true)]
async fn cancelling_returns_the_windows_and_cancelling_nothing_says_nothing() {
    let (handle, _) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    let (window, decision) = clients[FOE].decision().await;
    clients[FOE].drain();
    clients[ME].drain();

    send(&handle, &clients[ME], ClientMessage::CancelYield).await;
    silent(&clients[ME].drain());

    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    yield_until(&handle, &clients[ME], YieldUntil::MyTurn).await;
    assert_eq!(
        notices(&clients[ME].drain()),
        [Some(YieldUntil::EndOfTurn), Some(YieldUntil::MyTurn)],
        "a new yield replaces the old one"
    );
    yield_until(&handle, &clients[ME], YieldUntil::MyTurn).await;
    silent(&clients[ME].drain());

    send(&handle, &clients[ME], ClientMessage::CancelYield).await;
    assert_eq!(order(&clients[ME].drain()), ["yield none"]);
    silent(&clients[FOE].drain());

    // The windows are back: a spell of seat 0 is asked of seat 1, which can answer it.
    foe_casts_at(&handle, &mut clients[FOE], window, &decision).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;
    let told = clients[ME].drain();
    assert_eq!(priority_decisions(&told), 1);
}

#[tokio::test(start_paused = true)]
async fn each_seat_has_its_own_yield() {
    let (handle, _) = spawn(table_setup(3, 5), OFF);
    let mut clients = join_and_ready(&handle, 3).await;
    yield_until(&handle, &clients[2], YieldUntil::MyTurn).await;
    assert_eq!(notices(&clients[2].drain()), [Some(YieldUntil::MyTurn)]);
    assert_eq!(
        notices(&clients[0].drain()),
        [],
        "another seat hears nothing"
    );
    assert_eq!(notices(&clients[1].drain()), []);

    // A welcome tells a reconnecting seat what the server holds for it, and only for it.
    let mut back = Client::join(&handle, 2).await;
    assert!(matches!(
        back.next().await,
        ServerMessage::Welcome {
            yielding: Some(YieldUntil::MyTurn),
            ..
        }
    ));
    let mut other = Client::join(&handle, 1).await;
    assert!(matches!(
        other.next().await,
        ServerMessage::Welcome { yielding: None, .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn a_game_resumed_from_its_log_starts_without_yields() {
    let setup = combat_duel();
    let store = MemoryStore::new();
    let first = spawn_game_with(game_id(1), setup.clone(), store.clone(), config(OFF)).unwrap();
    let mut clients = join_and_ready(&first, 2).await;
    yield_until(&first, &clients[ME], YieldUntil::EndOfTurn).await;

    // Seat 0 attacks; seat 1 blocks nothing, and its yield passes its priority in the declare blockers step.
    let (id, decision) = clients[FOE].decision().await;
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("expected the attack, got {decision:?}");
    };
    let attack = vec![(
        options.attackers[0].object,
        AttackTarget::Player(PlayerId(1)),
    )];
    answer(&first, &clients[FOE], id, Answer::DeclareAttackers(attack)).await;
    foe_passes_once(&first, &mut clients[FOE]).await;
    let (blocks, _) = clients[ME].decision().await;
    answer(
        &first,
        &clients[ME],
        blocks,
        Answer::DeclareBlockers(Vec::new()),
    )
    .await;
    assert_eq!(first.status().await.unwrap().waiting_on, Some(seat(FOE)));
    drop(clients);
    drop(first);

    // The game resumes with every seat in the default mode and no yield: seat 0 holds nothing it can use and is
    // passed for at start-up, so seat 1, which holds an instant, is asked in the declare blockers step.
    let second = resume_game_with(store.clone(), game_id(1), setup, config(OFF))
        .await
        .unwrap();
    assert_eq!(second.status().await.unwrap().waiting_on, Some(seat(ME)));
    let mut me = Client::join(&second, ME).await;
    assert!(matches!(
        me.next().await,
        ServerMessage::Welcome { yielding: None, .. }
    ));
    let (_, decision) = me.decision().await;
    assert_eq!(decision.kind.name(), "priority");
    assert_eq!(notices(&me.drain()), []);
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_concedes_loses_its_yield_and_cannot_start_another() {
    let (handle, _) = spawn(table_setup(3, 5), OFF);
    let mut clients = join_and_ready(&handle, 3).await;
    yield_until(&handle, &clients[2], YieldUntil::MyTurn).await;
    clients[2].drain();

    send(&handle, &clients[2], ClientMessage::Concede).await;
    let told = clients[2].drain();
    assert_eq!(notices(&told), [None]);
    let tail = order(&told);
    assert!(
        tail.iter().position(|s| s == "yield none") > tail.iter().position(|s| s == "events"),
        "after the events of the concession: {tail:?}"
    );

    send(
        &handle,
        &clients[2],
        ClientMessage::Yield {
            until: YieldUntil::MyTurn,
        },
    )
    .await;
    assert!(is_error(&clients[2].drain(), ErrorCode::BadMessage));
    send(&handle, &clients[2], ClientMessage::CancelYield).await;
    silent(&clients[2].drain());
}

#[tokio::test(start_paused = true)]
async fn a_seat_that_loses_the_game_is_told_its_yield_ended_before_the_game_over() {
    // Seat 1 has one life and no creature to block with; it yields to the end of the turn.
    let setup = Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Raider", 2, 2))])
        .life(1, 1)
        .setup()
        .unwrap();
    let (handle, _) = spawn(setup, OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    clients[ME].drain();

    let (id, decision) = clients[FOE].decision().await;
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("expected the attack");
    };
    let attack = vec![(
        options.attackers[0].object,
        AttackTarget::Player(PlayerId(1)),
    )];
    answer(&handle, &clients[FOE], id, Answer::DeclareAttackers(attack)).await;
    pass_while_waiting(&handle, &mut clients[FOE]).await;

    let told = clients[ME].drain();
    let tail: Vec<_> = order(&told).into_iter().rev().take(2).collect();
    assert_eq!(tail, ["game over", "yield none"]);
    assert_eq!(
        handle.status().await.unwrap().result,
        Some(GameResult::Winner(PlayerId(0)))
    );
    send(
        &handle,
        &clients[ME],
        ClientMessage::Yield {
            until: YieldUntil::Resolve,
        },
    )
    .await;
    assert!(is_error(&clients[ME].drain(), ErrorCode::BadMessage));
}

#[tokio::test(start_paused = true)]
async fn a_yield_before_the_game_starts_is_refused() {
    let (handle, _) = spawn(instant_duel(), OFF);
    let mut client = Client::join(&handle, ME).await;
    // The welcome, and the roster that a lobby sends after it.
    assert!(matches!(client.next().await, ServerMessage::Welcome { .. }));
    assert!(matches!(client.next().await, ServerMessage::Lobby { .. }));
    send(
        &handle,
        &client,
        ClientMessage::Yield {
            until: YieldUntil::EndOfTurn,
        },
    )
    .await;
    assert!(is_error(&client.drain(), ErrorCode::BadMessage));
    send(&handle, &client, ClientMessage::CancelYield).await;
    silent(&client.drain());
}

#[tokio::test(start_paused = true)]
async fn a_failed_save_takes_back_a_new_yield_and_the_passes_it_made() {
    let (handle, store) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    foe_casts(&handle, &mut clients[FOE]).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;
    let (window, decision) = clients[ME].decision().await;
    clients[ME].drain();
    let before = handle.status().await.unwrap();

    store.fail(true);
    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    assert!(is_error(&clients[ME].drain(), ErrorCode::Internal));
    assert_eq!(handle.status().await.unwrap(), before, "nothing happened");

    // The window is still the seat's own, and the yield was not kept.
    store.fail(false);
    answer(&handle, &clients[ME], window, passive(&decision)).await;
    let told = clients[ME].drain();
    assert_eq!(notices(&told), []);
    assert!(handle.status().await.unwrap().events > before.events);
}

#[tokio::test(start_paused = true)]
async fn a_failed_save_takes_back_the_end_of_a_yield_too() {
    let (handle, store) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    yield_until(&handle, &clients[ME], YieldUntil::EndOfTurn).await;
    foe_casts(&handle, &mut clients[FOE]).await;
    clients[ME].drain();
    let before = handle.status().await.unwrap();

    // Seat 0's pass would make the new spell ready for seat 1, ending its yield; the save fails.
    store.fail(true);
    let (id, decision) = clients[FOE].decision().await;
    answer(&handle, &clients[FOE], id, passive(&decision)).await;
    assert!(is_error(&clients[FOE].drain(), ErrorCode::Internal));
    silent(&clients[ME].drain());
    assert_eq!(handle.status().await.unwrap(), before);

    // Once it can be saved the same pass ends the yield, as it would have.
    store.fail(false);
    answer(&handle, &clients[FOE], id, passive(&decision)).await;
    let told = clients[ME].drain();
    assert_eq!(notices(&told), [None]);
    assert_eq!(priority_decisions(&told), 1);
}

#[tokio::test(start_paused = true)]
async fn a_late_answer_to_a_window_the_yield_passed_is_stale_and_changes_nothing() {
    let (handle, _) = spawn(instant_duel(), OFF);
    let mut clients = join_and_ready(&handle, 2).await;
    foe_casts(&handle, &mut clients[FOE]).await;
    foe_passes_once(&handle, &mut clients[FOE]).await;
    let (window, decision) = clients[ME].decision().await;
    yield_until(&handle, &clients[ME], YieldUntil::MyTurn).await;
    clients[ME].drain();
    let status = handle.status().await.unwrap();

    answer(&handle, &clients[ME], window, passive(&decision)).await;
    assert!(is_error(&clients[ME].drain(), ErrorCode::StaleDecision));
    assert_eq!(handle.status().await.unwrap(), status);
}

/// Plays a whole game of bots, one of which yields to the end of the turn whenever it would pass.
async fn bots_with_a_yielding_seat(players: usize, seed: u64, pace: Pace, until: YieldUntil) {
    let setup = table_setup(players, seed);
    let store = MemoryStore::new();
    let handle = spawn_game_with(game_id(1), setup.clone(), store.clone(), config(pace)).unwrap();
    let mut bots = Vec::new();
    for n in 0..players {
        let mut bot = Bot::join(&handle, n, seed).await;
        if n == 0 {
            bot.yield_on_pass = Some(until);
        }
        bots.push(bot);
    }
    let reports = common::play_bots(&handle, bots).await;

    let result = handle
        .status()
        .await
        .unwrap()
        .result
        .expect("the game ended");
    assert!(reports.iter().all(|r| r.result == result));
    assert!(reports.iter().all(|r| r.errors.is_empty()), "{reports:?}");
    let yielder = &reports[0];
    assert!(
        yielder.yields > 1,
        "the bot yielded {} times",
        yielder.yields
    );

    // The log is the normal one: it replays to the state the actor ended in, and the seat's passes were made for it.
    let log = store.load(game_id(1)).await.unwrap();
    let status = handle.status().await.unwrap();
    assert_eq!(
        replay(&setup, &log).unwrap().state_hash(),
        status.state_hash.unwrap()
    );
    assert!(
        passes_by(&log, 0) > yielder.yields,
        "a yield passes more than once"
    );
}

#[rstest]
#[tokio::test]
async fn a_game_where_a_seat_yields_to_the_end_of_every_turn_ends_normally(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 2, 3)] seed: u64,
    #[values(OFF, DEFAULT)] pace: Pace,
) {
    bots_with_a_yielding_seat(players, seed, pace, YieldUntil::EndOfTurn).await;
}

// `Resolve` is left to the tests above: the bots hold no instants, so a seat that can act is asked over a spell only
// in its own main phase, and a bot rarely stays able to act after casting one.
#[rstest]
#[tokio::test]
async fn so_does_one_that_yields_until_its_own_turn(
    #[values(2, 4)] players: usize,
    #[values(1, 2, 3, 4)] seed: u64,
) {
    bots_with_a_yielding_seat(players, seed, OFF, YieldUntil::MyTurn).await;
}
